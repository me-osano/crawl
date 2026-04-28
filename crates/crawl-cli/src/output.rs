use anyhow::Result;
use comfy_table::{Table, presets, Cell, Attribute};
use owo_colors::OwoColorize;
use serde_json::Value;

pub struct CliRenderable {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl CliRenderable {
    pub fn new(headers: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        Self { headers, rows }
    }

    pub fn render(&self) -> String {
        let mut table = Table::new();
        table.load_preset(presets::UTF8_FULL);

        let header_cells: Vec<Cell> = self.headers
            .iter()
            .map(|h| Cell::new(h).add_attribute(Attribute::Bold))
            .collect();
        table.set_header(header_cells);

        for row in &self.rows {
            let cells: Vec<Cell> = row
                .iter()
                .map(|c| Cell::new(c).fg(comfy_table::Color::Cyan))
                .collect();
            table.add_row(cells);
        }

        table.to_string()
    }
}

pub fn print_value(val: &Value, pretty: bool) {
    if pretty {
        println!("{}", serde_json::to_string_pretty(val).unwrap_or_default());
    } else {
        println!("{}", val);
    }
}

pub fn print_ok(msg: &str) {
    println!("{} {}", "✓".green().bold(), msg.green());
}

pub fn print_info(msg: &str) {
    println!("{} {}", "›".cyan().bold(), msg.cyan());
}

pub fn print_err(msg: &str) {
    eprintln!("✗ {msg}");
}

pub fn handle_format(value: &Value, json_mode: bool, human_fn: impl FnOnce(&Value) -> Result<()>) -> Result<()> {
    if json_mode {
        print_value(value, true);
        Ok(())
    } else {
        human_fn(value)
    }
}

pub fn render_table(renderable: &CliRenderable) {
    println!("{}", renderable.render());
}

pub fn print_header(title: &str) {
    println!();
    println!("{}", title.bold().white());
    println!("{}", "-".repeat(title.len()).dimmed());
}