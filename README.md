# oxc-miette

`oxc-miette` provides the diagnostic protocol and renderers used by Oxc.

The crate contains:

- `Diagnostic` and `SourceCode` traits
- source spans, labels, severities, and named sources
- graphical and JSON diagnostic renderers

It intentionally does not provide an application error container or implicit
global rendering. Applications own diagnostics directly—typically as
`Box<dyn Diagnostic + Send + Sync>`—and select a renderer explicitly.

```rust
use std::fmt;

use miette::{Diagnostic, GraphicalReportHandler, GraphicalTheme};

#[derive(Debug)]
struct Example;

impl fmt::Display for Example {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("something went wrong")
    }
}

impl std::error::Error for Example {}
impl Diagnostic for Example {}

let mut output = String::new();
GraphicalReportHandler::new_themed(GraphicalTheme::none())
    .render_report(&mut output, &Example)
    .unwrap();
assert!(output.contains("something went wrong"));
```

## License

Apache-2.0
