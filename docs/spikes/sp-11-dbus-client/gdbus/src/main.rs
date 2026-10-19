//! Reads the monitors Mutter reports by running `gdbus call` and scanning its text output.

use std::process::Command;

/// The four quoted strings that start each monitor: `(('HDMI-1', 'PHL', 'PHL 242V8', '0x00008fac'), [`.
fn monitors(text: &str) -> Vec<[String; 4]> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("(('") {
        rest = &rest[at + 2..];
        let mut fields: Vec<String> = Vec::new();
        let mut cursor = rest;
        while fields.len() < 4 {
            let Some(start) = cursor.find('\'') else { break };
            let mut value = String::new();
            let mut chars = cursor[start + 1..].char_indices();
            let mut end = cursor.len();
            while let Some((i, ch)) = chars.next() {
                match ch {
                    '\\' => value.extend(chars.next().map(|(_, c)| c)),
                    '\'' => {
                        end = start + 1 + i + 1;
                        break;
                    }
                    other => value.push(other),
                }
            }
            fields.push(value);
            cursor = &cursor[end..];
        }
        if let [a, b, c, d] = fields.as_slice() {
            found.push([a.clone(), b.clone(), c.clone(), d.clone()]);
        }
    }
    found
}

fn main() {
    let output = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.gnome.Mutter.DisplayConfig",
            "--object-path",
            "/org/gnome/Mutter/DisplayConfig",
            "--method",
            "org.gnome.Mutter.DisplayConfig.GetCurrentState",
        ])
        .output()
        .expect("gdbus runs");
    for [connector, vendor, product, serial] in monitors(&String::from_utf8_lossy(&output.stdout)) {
        println!("{connector}  {vendor}:{product}:{serial}");
    }
}
