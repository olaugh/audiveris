// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact object-array TimSort control flow used by OpenJDK 25.
//!
//! STEMS builder comparators are intentionally pair-dependent and can contain
//! cycles, so substituting Rust's stable sort changes observable permutations.

use std::cmp::Ordering;

const MIN_MERGE: usize = 32;
const MIN_GALLOP: i32 = 7;

/// Sort `values` with OpenJDK 25's object `TimSort` control flow.
///
/// Returns `false` only where Java throws `IllegalArgumentException` because
/// the comparator violates its general contract during a merge.
pub(crate) fn sort_by<T, F>(values: &mut [T], compare: F) -> bool
where
    T: Clone,
    F: Fn(&T, &T) -> Ordering + Copy,
{
    let mut sort = Jdk25TimSort {
        items: values.to_vec(),
        compare,
        min_gallop: MIN_GALLOP,
        run_base: Vec::new(),
        run_len: Vec::new(),
    };
    if !sort.sort() {
        return false;
    }
    values.clone_from_slice(&sort.items);
    true
}

struct Jdk25TimSort<T, F> {
    items: Vec<T>,
    compare: F,
    min_gallop: i32,
    run_base: Vec<usize>,
    run_len: Vec<usize>,
}

impl<T, F> Jdk25TimSort<T, F>
where
    T: Clone,
    F: Fn(&T, &T) -> Ordering + Copy,
{
    fn sort(&mut self) -> bool {
        let mut remaining = self.items.len();
        if remaining < 2 {
            return true;
        }
        if remaining < MIN_MERGE {
            let run = self.count_run_and_make_ascending(0, remaining);
            self.binary_sort(0, remaining, run);
            return true;
        }

        let min_run = min_run_length(remaining);
        let mut lo = 0;
        while remaining != 0 {
            let mut run = self.count_run_and_make_ascending(lo, self.items.len());
            if run < min_run {
                let force = remaining.min(min_run);
                self.binary_sort(lo, lo + force, lo + run);
                run = force;
            }
            self.push_run(lo, run);
            if !self.merge_collapse() {
                return false;
            }
            lo += run;
            remaining -= run;
        }
        self.merge_force_collapse()
    }

    fn cmp(&self, left: &T, right: &T) -> Ordering {
        (self.compare)(left, right)
    }

    fn binary_sort(&mut self, lo: usize, hi: usize, mut start: usize) {
        if start == lo {
            start += 1;
        }
        while start < hi {
            let pivot = self.items[start].clone();
            let mut left = lo;
            let mut right = start;
            while left < right {
                let middle = left + (right - left) / 2;
                if self.cmp(&pivot, &self.items[middle]) == Ordering::Less {
                    right = middle;
                } else {
                    left = middle + 1;
                }
            }
            move_within(&mut self.items, left, start - left, left + 1);
            self.items[left] = pivot;
            start += 1;
        }
    }

    fn count_run_and_make_ascending(&mut self, lo: usize, hi: usize) -> usize {
        let mut run_hi = lo + 1;
        if run_hi == hi {
            return 1;
        }
        if self.cmp(&self.items[run_hi], &self.items[lo]) == Ordering::Less {
            run_hi += 1;
            while run_hi < hi
                && self.cmp(&self.items[run_hi], &self.items[run_hi - 1]) == Ordering::Less
            {
                run_hi += 1;
            }
            self.items[lo..run_hi].reverse();
        } else {
            run_hi += 1;
            while run_hi < hi
                && self.cmp(&self.items[run_hi], &self.items[run_hi - 1]) != Ordering::Less
            {
                run_hi += 1;
            }
        }
        run_hi - lo
    }

    fn push_run(&mut self, base: usize, len: usize) {
        self.run_base.push(base);
        self.run_len.push(len);
    }

    fn merge_collapse(&mut self) -> bool {
        while self.run_len.len() > 1 {
            let mut n = self.run_len.len() - 2;
            if (n > 0 && self.run_len[n - 1] <= self.run_len[n] + self.run_len[n + 1])
                || (n > 1 && self.run_len[n - 2] <= self.run_len[n] + self.run_len[n - 1])
            {
                if self.run_len[n - 1] < self.run_len[n + 1] {
                    n -= 1;
                }
            } else if self.run_len[n] > self.run_len[n + 1] {
                break;
            }
            if !self.merge_at(n) {
                return false;
            }
        }
        true
    }

    fn merge_force_collapse(&mut self) -> bool {
        while self.run_len.len() > 1 {
            let mut n = self.run_len.len() - 2;
            if n > 0 && self.run_len[n - 1] < self.run_len[n + 1] {
                n -= 1;
            }
            if !self.merge_at(n) {
                return false;
            }
        }
        true
    }

    fn merge_at(&mut self, index: usize) -> bool {
        let mut base1 = self.run_base[index];
        let mut len1 = self.run_len[index];
        let base2 = self.run_base[index + 1];
        let mut len2 = self.run_len[index + 1];
        self.run_len[index] = len1 + len2;
        if self.run_len.len() >= 3 && index == self.run_len.len() - 3 {
            self.run_base[index + 1] = self.run_base[index + 2];
            self.run_len[index + 1] = self.run_len[index + 2];
        }
        self.run_base.pop();
        self.run_len.pop();

        let key2 = self.items[base2].clone();
        let skipped = gallop_right(&key2, &self.items, base1, len1, 0, self.compare);
        base1 += skipped;
        len1 -= skipped;
        if len1 == 0 {
            return true;
        }
        let key1 = self.items[base1 + len1 - 1].clone();
        len2 = gallop_left(&key1, &self.items, base2, len2, len2 - 1, self.compare);
        if len2 == 0 {
            return true;
        }
        if len1 <= len2 {
            self.merge_lo(base1, len1, base2, len2)
        } else {
            self.merge_hi(base1, len1, base2, len2)
        }
    }

    fn merge_lo(&mut self, base1: usize, mut len1: usize, base2: usize, mut len2: usize) -> bool {
        let temporary = self.items[base1..base1 + len1].to_vec();
        let mut cursor1 = 0;
        let mut cursor2 = base2;
        let mut destination = base1;

        self.items[destination] = self.items[cursor2].clone();
        destination += 1;
        cursor2 += 1;
        len2 -= 1;
        if len2 == 0 {
            self.items[destination..destination + len1]
                .clone_from_slice(&temporary[cursor1..cursor1 + len1]);
            return true;
        }
        if len1 == 1 {
            move_within(&mut self.items, cursor2, len2, destination);
            self.items[destination + len2] = temporary[cursor1].clone();
            return true;
        }

        let mut min_gallop = self.min_gallop;
        'outer: loop {
            let mut count1 = 0;
            let mut count2 = 0;
            loop {
                if (self.compare)(&self.items[cursor2], &temporary[cursor1]) == Ordering::Less {
                    self.items[destination] = self.items[cursor2].clone();
                    destination += 1;
                    cursor2 += 1;
                    count2 += 1;
                    count1 = 0;
                    len2 -= 1;
                    if len2 == 0 {
                        break 'outer;
                    }
                } else {
                    self.items[destination] = temporary[cursor1].clone();
                    destination += 1;
                    cursor1 += 1;
                    count1 += 1;
                    count2 = 0;
                    len1 -= 1;
                    if len1 == 1 {
                        break 'outer;
                    }
                }
                if (count1 | count2) >= min_gallop {
                    break;
                }
            }

            loop {
                let key = self.items[cursor2].clone();
                count1 = gallop_right(&key, &temporary, cursor1, len1, 0, self.compare) as i32;
                if count1 != 0 {
                    let count = count1 as usize;
                    self.items[destination..destination + count]
                        .clone_from_slice(&temporary[cursor1..cursor1 + count]);
                    destination += count;
                    cursor1 += count;
                    len1 -= count;
                    if len1 <= 1 {
                        break 'outer;
                    }
                }
                self.items[destination] = self.items[cursor2].clone();
                destination += 1;
                cursor2 += 1;
                len2 -= 1;
                if len2 == 0 {
                    break 'outer;
                }

                let key = temporary[cursor1].clone();
                count2 = gallop_left(&key, &self.items, cursor2, len2, 0, self.compare) as i32;
                if count2 != 0 {
                    let count = count2 as usize;
                    move_within(&mut self.items, cursor2, count, destination);
                    destination += count;
                    cursor2 += count;
                    len2 -= count;
                    if len2 == 0 {
                        break 'outer;
                    }
                }
                self.items[destination] = temporary[cursor1].clone();
                destination += 1;
                cursor1 += 1;
                len1 -= 1;
                if len1 == 1 {
                    break 'outer;
                }
                min_gallop -= 1;
                if count1 < MIN_GALLOP && count2 < MIN_GALLOP {
                    break;
                }
            }
            min_gallop = min_gallop.max(0) + 2;
        }
        self.min_gallop = min_gallop.max(1);

        if len1 == 1 {
            move_within(&mut self.items, cursor2, len2, destination);
            self.items[destination + len2] = temporary[cursor1].clone();
            true
        } else if len1 == 0 {
            false
        } else {
            self.items[destination..destination + len1]
                .clone_from_slice(&temporary[cursor1..cursor1 + len1]);
            true
        }
    }

    fn merge_hi(&mut self, base1: usize, mut len1: usize, base2: usize, mut len2: usize) -> bool {
        let temporary = self.items[base2..base2 + len2].to_vec();
        let mut cursor1 = (base1 + len1 - 1) as isize;
        let mut cursor2 = (len2 - 1) as isize;
        let mut destination = (base2 + len2 - 1) as isize;

        self.items[destination as usize] = self.items[cursor1 as usize].clone();
        destination -= 1;
        cursor1 -= 1;
        len1 -= 1;
        if len1 == 0 {
            let start = (destination - (len2 as isize - 1)) as usize;
            self.items[start..start + len2].clone_from_slice(&temporary[..len2]);
            return true;
        }
        if len2 == 1 {
            destination -= len1 as isize;
            cursor1 -= len1 as isize;
            move_within(
                &mut self.items,
                (cursor1 + 1) as usize,
                len1,
                (destination + 1) as usize,
            );
            self.items[destination as usize] = temporary[cursor2 as usize].clone();
            return true;
        }

        let mut min_gallop = self.min_gallop;
        'outer: loop {
            let mut count1 = 0;
            let mut count2 = 0;
            loop {
                if (self.compare)(&temporary[cursor2 as usize], &self.items[cursor1 as usize])
                    == Ordering::Less
                {
                    self.items[destination as usize] = self.items[cursor1 as usize].clone();
                    destination -= 1;
                    cursor1 -= 1;
                    count1 += 1;
                    count2 = 0;
                    len1 -= 1;
                    if len1 == 0 {
                        break 'outer;
                    }
                } else {
                    self.items[destination as usize] = temporary[cursor2 as usize].clone();
                    destination -= 1;
                    cursor2 -= 1;
                    count2 += 1;
                    count1 = 0;
                    len2 -= 1;
                    if len2 == 1 {
                        break 'outer;
                    }
                }
                if (count1 | count2) >= min_gallop {
                    break;
                }
            }

            loop {
                let key = temporary[cursor2 as usize].clone();
                count1 = (len1
                    - gallop_right(&key, &self.items, base1, len1, len1 - 1, self.compare))
                    as i32;
                if count1 != 0 {
                    let count = count1 as usize;
                    destination -= count as isize;
                    cursor1 -= count as isize;
                    len1 -= count;
                    move_within(
                        &mut self.items,
                        (cursor1 + 1) as usize,
                        count,
                        (destination + 1) as usize,
                    );
                    if len1 == 0 {
                        break 'outer;
                    }
                }
                self.items[destination as usize] = temporary[cursor2 as usize].clone();
                destination -= 1;
                cursor2 -= 1;
                len2 -= 1;
                if len2 == 1 {
                    break 'outer;
                }

                let key = self.items[cursor1 as usize].clone();
                count2 =
                    (len2 - gallop_left(&key, &temporary, 0, len2, len2 - 1, self.compare)) as i32;
                if count2 != 0 {
                    let count = count2 as usize;
                    destination -= count as isize;
                    cursor2 -= count as isize;
                    len2 -= count;
                    self.items[(destination + 1) as usize..(destination + 1) as usize + count]
                        .clone_from_slice(
                            &temporary[(cursor2 + 1) as usize..(cursor2 + 1) as usize + count],
                        );
                    if len2 <= 1 {
                        break 'outer;
                    }
                }
                self.items[destination as usize] = self.items[cursor1 as usize].clone();
                destination -= 1;
                cursor1 -= 1;
                len1 -= 1;
                if len1 == 0 {
                    break 'outer;
                }
                min_gallop -= 1;
                if count1 < MIN_GALLOP && count2 < MIN_GALLOP {
                    break;
                }
            }
            min_gallop = min_gallop.max(0) + 2;
        }
        self.min_gallop = min_gallop.max(1);

        if len2 == 1 {
            destination -= len1 as isize;
            cursor1 -= len1 as isize;
            move_within(
                &mut self.items,
                (cursor1 + 1) as usize,
                len1,
                (destination + 1) as usize,
            );
            self.items[destination as usize] = temporary[cursor2 as usize].clone();
            true
        } else if len2 == 0 {
            false
        } else {
            let start = (destination - (len2 as isize - 1)) as usize;
            self.items[start..start + len2].clone_from_slice(&temporary[..len2]);
            true
        }
    }
}

fn min_run_length(mut value: usize) -> usize {
    let mut remainder = 0;
    while value >= MIN_MERGE {
        remainder |= value & 1;
        value >>= 1;
    }
    value + remainder
}

fn move_within<T: Clone>(values: &mut [T], source: usize, length: usize, destination: usize) {
    if length == 0 || source == destination {
        return;
    }
    let moved = values[source..source + length].to_vec();
    values[destination..destination + length].clone_from_slice(&moved);
}

fn gallop_left<T, F>(
    key: &T,
    values: &[T],
    base: usize,
    len: usize,
    hint: usize,
    compare: F,
) -> usize
where
    F: Fn(&T, &T) -> Ordering + Copy,
{
    let mut last_offset: isize = 0;
    let mut offset: isize = 1;
    if compare(key, &values[base + hint]) == Ordering::Greater {
        let max_offset = (len - hint) as isize;
        while offset < max_offset
            && compare(
                key,
                &values[(base as isize + hint as isize + offset) as usize],
            ) == Ordering::Greater
        {
            last_offset = offset;
            offset = (offset << 1) + 1;
        }
        offset = offset.min(max_offset);
        last_offset += hint as isize;
        offset += hint as isize;
    } else {
        let max_offset = (hint + 1) as isize;
        while offset < max_offset
            && compare(
                key,
                &values[(base as isize + hint as isize - offset) as usize],
            ) != Ordering::Greater
        {
            last_offset = offset;
            offset = (offset << 1) + 1;
        }
        offset = offset.min(max_offset);
        let previous = last_offset;
        last_offset = hint as isize - offset;
        offset = hint as isize - previous;
    }
    last_offset += 1;
    while last_offset < offset {
        let middle = last_offset + (offset - last_offset) / 2;
        if compare(key, &values[(base as isize + middle) as usize]) == Ordering::Greater {
            last_offset = middle + 1;
        } else {
            offset = middle;
        }
    }
    offset as usize
}

fn gallop_right<T, F>(
    key: &T,
    values: &[T],
    base: usize,
    len: usize,
    hint: usize,
    compare: F,
) -> usize
where
    F: Fn(&T, &T) -> Ordering + Copy,
{
    let mut offset: isize = 1;
    let mut last_offset: isize = 0;
    if compare(key, &values[base + hint]) == Ordering::Less {
        let max_offset = (hint + 1) as isize;
        while offset < max_offset
            && compare(
                key,
                &values[(base as isize + hint as isize - offset) as usize],
            ) == Ordering::Less
        {
            last_offset = offset;
            offset = (offset << 1) + 1;
        }
        offset = offset.min(max_offset);
        let previous = last_offset;
        last_offset = hint as isize - offset;
        offset = hint as isize - previous;
    } else {
        let max_offset = (len - hint) as isize;
        while offset < max_offset
            && compare(
                key,
                &values[(base as isize + hint as isize + offset) as usize],
            ) != Ordering::Less
        {
            last_offset = offset;
            offset = (offset << 1) + 1;
        }
        offset = offset.min(max_offset);
        last_offset += hint as isize;
        offset += hint as isize;
    }
    last_offset += 1;
    while last_offset < offset {
        let middle = last_offset + (offset - last_offset) / 2;
        if compare(key, &values[(base as isize + middle) as usize]) == Ordering::Less {
            offset = middle;
        } else {
            last_offset = middle + 1;
        }
    }
    offset as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_path_matches_stable_integer_order() {
        let mut values = (0..64).rev().collect::<Vec<_>>();
        assert!(sort_by(&mut values, Ord::cmp));
        assert_eq!(values, (0..64).collect::<Vec<_>>());
    }

    #[test]
    fn merge_path_is_stable() {
        let mut values = (0..40)
            .rev()
            .map(|ordinal| (ordinal % 5, ordinal))
            .collect::<Vec<_>>();
        assert!(sort_by(&mut values, |left, right| left.0.cmp(&right.0)));
        for key in 0..5 {
            assert_eq!(
                values
                    .iter()
                    .filter(|value| value.0 == key)
                    .map(|value| value.1)
                    .collect::<Vec<_>>(),
                (0..40)
                    .rev()
                    .filter(|ordinal| ordinal % 5 == key)
                    .collect::<Vec<_>>()
            );
        }
    }
}
