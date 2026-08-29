# Clean REDUCTION whole-note fixture

`piano-disconnected-barlines.mei` contains exactly 42 notated heads. The final
two are stemless whole notes. Java Audiveris retains all 42 through REDUCTION;
the live Java SIG geometry at each raster scale is frozen in
`java-reduction-heads.txt`.

The PNGs were rendered with Verovio 6.2.1's Bravura font and librsvg on a white
background. Run `./regenerate.sh` with those tools to reproduce them. Expected
SHA-256 digests are:

```text
4e5c65832d1ff1dad19014b2e92cd74176e495e64af49b6109007873411ced22  disconnected-1x.png
a230189f18dd3b7dac09b4cc2257d4d8dde22b87d28d4fda1437b53f2c43522c  disconnected-1_5x.png
a6c6d9ff8ff79695bf30f2431f3537ab5a5566d9cb6be1da1436b5267adf9288  disconnected-2x.png
```

The Java oracle was captured after `OmrStep.REDUCTION` from each page's live
`SIGraph`, excluding removed inters and selecting `HeadInter` instances. The
Rust integration test compares the complete sorted shape/bounds set, so a
missing true head cannot be hidden by an extra false-positive survivor.
