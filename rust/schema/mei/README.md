<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# MEI 5.1 CMN schema provenance

This directory vendors the official Music Encoding Initiative 5.1 Common Music
Notation Relax NG schema. It intentionally uses the CMN customization, not the
broader `mei-all` schema. The RNG is copied byte-for-byte from the official
source; CI validates its pinned digest and never fetches a replacement.

| Property | Value |
| --- | --- |
| File | `mei-CMN-5.1.rng` |
| MEI release | 5.1 |
| Customization | Common Music Notation (CMN) |
| Official source | <https://music-encoding.org/schema/5.1/mei-CMN.rng> |
| Downloaded | 2026-08-09 |
| SHA-256 | `f6440d5eb59c3e903f2a7a64ea26518646186f8449b86096106846a269eb354b` |
| License | Educational Community License 2.0 (`ECL-2.0`) |

The schema's own header identifies MEI 5.1, records an ODD generation timestamp
of `2025-01-22T21:19:13Z`, and declares the ECL-2.0 license. The unmodified
license text is retained as `LICENSE-ECL-2.0.txt`; its canonical SPDX source is
<https://spdx.org/licenses/ECL-2.0.txt> and its vendored SHA-256 is
`f8303e68f99ec4056b4833dd3626ea661adf443bf0b8fbbebea0856898230db4`.

The generated schema is copyright the Music Encoding Initiative (MEI) Board.
Its source is a derivative of earlier schema versions copyright 2001–2006
Perry Roland and the Rector and Visitors of the University of Virginia under
ECL-1.0. The vendored RNG remains third-party ECL-2.0 material; the workspace's
AGPL-3.0-or-later license does not relicense it.

`rust/scripts/validate-mei-schema.sh` checks the schema digest before using
`xmllint` to validate every checked-in `.mei` golden below
`rust/crates/audiveris-mei/tests`. A schema update must be deliberate: download
the new official customization, review its release and license, update the
filename and both pinned digests, and regenerate or revalidate every golden.
The command performs Relax NG validation. Although the generated RNG carries
some Schematron declarations, `xmllint --relaxng` does not execute those
Schematron assertions, so this gate does not claim that additional coverage.
