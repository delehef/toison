use anyhow::*;
use clap::{Parser, ValueEnum};
use colored::Colorize;
use human_format::*;
use serde_json::Value;
use thousands::Separable;

#[derive(Copy, Clone, Debug, ValueEnum)]
enum Count {
    Bytes,
    Children,
}
impl Count {
    fn format(&self, x: usize) -> String {
        match self {
            Count::Bytes => Formatter::new()
                .with_scales(Scales::Binary())
                .with_suffix("B")
                .format(x as f64),
            Count::Children => Formatter::new().with_scales(Scales::SI()).format(x as f64),
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    json_file: String,

    #[arg(
        short,
        long,
        default_value_t = 0.0,
        help = "hide nodes under this percentge of the total size"
    )]
    threshold: f32,

    #[arg(short, long, value_enum, default_value_t = Count::Bytes, help="the unit with which to weight nodes")]
    unit: Count,
}

#[derive(Debug, Clone)]
struct Node {
    tag: Option<String>,
    len: usize,
    size_b: usize,
    size_c: usize,
    key_size: usize,
    children: Option<Vec<Node>>,
}
impl Node {
    fn from_json(n: &Value, ks: usize, tag: String) -> Node {
        match n {
            Value::Null => Node::leaf(ks, 0, tag),
            Value::Bool(_) => Node::leaf(ks, 4, tag),
            Value::Number(x) => Node::leaf(ks, x.to_string().len(), tag),
            Value::String(s) => Node::leaf(ks, s.len(), tag),
            Value::Array(children) => {
                let children = children
                    .iter()
                    .map(|c| Node::from_json(c, 0, String::new()))
                    .collect::<Vec<_>>();
                Node {
                    tag: Some(tag),
                    len: children.len(),
                    size_b: children.iter().map(|c| c.size_b).sum::<usize>(),
                    size_c: children.len() + children.iter().map(|c| c.size_c).sum::<usize>(),
                    key_size: children.iter().map(|c| c.key_size).sum::<usize>(),
                    children: None,
                }
            }
            Value::Object(_children) => {
                let children = _children
                    .iter()
                    .map(|(k, v)| Node::from_json(v, k.len(), k.clone()))
                    .collect::<Vec<_>>();
                Node {
                    tag: Some(tag),
                    len: 0,
                    size_b: children.iter().map(|c| c.size_b).sum::<usize>(),
                    size_c: children.len() + children.iter().map(|c| c.size_c).sum::<usize>(),
                    key_size: _children.keys().map(|k| k.len()).sum::<usize>(),
                    children: Some(children),
                }
            }
        }
    }

    fn size(&self, count: Count) -> usize {
        match count {
            Count::Bytes => self.size_b,
            Count::Children => self.size_c,
        }
    }

    fn leaf(key_size: usize, size: usize, tag: String) -> Node {
        Node {
            tag: if tag.is_empty() { None } else { Some(tag) },
            len: 0,
            size_b: size,
            size_c: 0,
            key_size,
            children: None,
        }
    }

    fn render(&self, total_size: usize, depth: usize, threshold: f32, count: Count, width: usize) {
        // 11 + 6 + 2 = 19 chars required for numbers
        // -> (WIDTH - 19)×2/3 for tagline
        // -> (WIDTH - 19)×1/3 for bar
        let w_tagline = ((width - 19) * 2) / 3;
        let w_bar = width - 19 - w_tagline - 2;

        let rel_size = self.size(count) as f32 / total_size as f32;
        if rel_size < threshold {
            return;
        }

        let indent = " ".repeat(2 * depth);
        let cardinality = if self.len > 0 {
            format!("[{}] ", self.len.to_string().separate_with_commas())
        } else {
            String::new()
        };
        let mut id = format!(
            "{}{}{}",
            indent,
            cardinality,
            &self.tag.clone().unwrap_or_default()
        );
        if id.len() > w_tagline {
            id = format!("{}…", id.chars().take(w_tagline - 2).collect::<String>());
        }

        let header = format!(
            "{:0w_tagline$} {:>6.2}% {:>11}",
            id,
            100. * rel_size,
            format!("({})", count.format(self.size(count))),
            w_tagline = w_tagline,
        );
        let color_factor = (155_f32 * rel_size) as u8;
        println!(
            "{:55} {}",
            header.truecolor(100 + color_factor, 100, 100),
            "▒".repeat((rel_size * w_bar as f32) as usize)
        );
        if let Some(children) = &self.children {
            for child in children {
                child.render(total_size, depth + 1, threshold, count, width);
            }
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    let root = Node::from_json(
        &serde_json::from_str(
            &std::fs::read_to_string(&args.json_file)
                .with_context(|| format!("while reading `{}`", args.json_file))?,
        )?,
        0,
        "Root".to_owned(),
    );

    let width = if let Some((w, _)) = term_size::dimensions() {
        w
    } else {
        100
    };

    root.render(
        root.size(args.unit),
        0,
        args.threshold / 100.,
        args.unit,
        width,
    );

    Ok(())
}
