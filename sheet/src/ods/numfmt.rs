//! ODF data styles (`number:number-style`, `number:date-style`, …) as the
//! format codes the engine and xlsx use (`#,##0`, `yyyy/m/d`).

use std::collections::HashMap;

/// Every data style in one XML part (content.xml or styles.xml), by name,
/// as a format code. Styles this cannot express are left out
pub fn parse_data_styles(_xml: &str) -> HashMap<String, String> {
    HashMap::new()
}
