// SPDX-License-Identifier: AGPL-3.0-or-later

pub fn generate<T: Clone>(items: &[T], bucket_size: usize) -> Vec<Vec<T>> {
    fn helper<T: Clone>(
        array: &mut [T],
        bucket_size: usize,
        start: usize,
        current: &mut Vec<T>,
        output: &mut Vec<Vec<T>>,
    ) {
        if current.len() == bucket_size {
            output.push(current.clone());
            return;
        }
        for index in start..array.len() {
            current.push(array[index].clone());
            array.swap(start, index);
            helper(array, bucket_size, start + 1, current, output);
            array.swap(start, index);
            current.pop();
        }
    }

    let mut array = items.to_vec();
    let mut output = Vec::new();
    helper(&mut array, bucket_size, 0, &mut Vec::new(), &mut output);
    output
}

pub fn reduce<T: PartialEq>(items: &mut Vec<Vec<T>>) {
    let mut index = 0;
    while index < items.len() {
        let mut other = index + 1;
        while other < items.len() {
            if items[index] == items[other] {
                items.remove(other);
            } else {
                other += 1;
            }
        }
        index += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_java_generation_order() {
        assert_eq!(
            generate(&[10, 20, 30], 2),
            vec![
                vec![10, 20],
                vec![10, 30],
                vec![20, 10],
                vec![20, 30],
                vec![30, 20],
                vec![30, 10]
            ]
        );
    }

    #[test]
    fn reduces_duplicates() {
        let mut values = vec![vec![1, 2], vec![1, 2], vec![2, 1]];
        reduce(&mut values);
        assert_eq!(values, vec![vec![1, 2], vec![2, 1]]);
    }
}
