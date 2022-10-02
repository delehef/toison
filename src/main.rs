use anyhow::*;
use clap::{Parser, ValueEnum};
use colored::{Color, Colorize};
use human_format::*;
use serde_json::Value;
use thousands::Separable;

#[derive(Copy, Clone, Debug, ValueEnum)]
enum Unit {
    Bytes,
    Children,
}
impl Unit {
    fn format(&self, x: usize) -> String {
        match self {
            Unit::Bytes => Formatter::new()
                .with_scales(Scales::Binary())
                .with_suffix("B")
                .format(x as f64),
            Unit::Children => Formatter::new().with_scales(Scales::SI()).format(x as f64),
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Colorizer {
    Hellscape,
    Gradient,
    Monochrome,
    None,
}
impl Colorizer {
    fn colorize(&self, rel: f32) -> Color {
        match self {
            Colorizer::Hellscape => {
                let rel_b = (155_f32 * rel) as u8;
                Color::TrueColor {
                    r: 100 + rel_b,
                    g: 100,
                    b: 100,
                }
            }
            Colorizer::Gradient => {
                let rel_b = (155_f32 * rel) as u8;
                Color::TrueColor {
                    r: 100 + rel_b,
                    g: 200 - rel_b,
                    b: 100,
                }
            }
            Colorizer::Monochrome => {
                let rel_b = (155_f32 * rel) as u8;
                Color::TrueColor {
                    r: 100 + rel_b,
                    g: 100 + rel_b,
                    b: 100 + rel_b,
                }
            }
            Colorizer::None => Color::White,
        }
    }
}

struct DisplaySettings {
    counter: Unit,
    colorizer: Colorizer,
    depth: Option<usize>,
    width: usize,
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

    #[arg(
        short,
        long,
        allow_negative_numbers = true,
        help = "the maximum depth to render; if negative, counts from the deepest node"
    )]
    max_depth: Option<isize>,

    #[arg(short, long, value_enum, default_value_t = Count::Bytes, help="the unit with which to weight nodes")]
    unit: Unit,

    #[arg(short, long, value_enum, default_value_t = Colorizer::Hellscape, help="how to colorize output")]
    colors: Colorizer,
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

    fn render(&self, total_size: usize, depth: usize, threshold: f32, settings: &DisplaySettings) {
        if let Some(max_depth) = settings.depth {
            if depth >= max_depth {
                return;
            }
        }
        // 11 + 6 + 2 = 19 chars required for numbers
        // -> (WIDTH - 19)×2/3 for tagline
        // -> (WIDTH - 19)×1/3 for bar
        let w_tagline = ((settings.width - 19) * 2) / 3;
        let w_bar = settings.width - 19 - w_tagline - 2;

        let rel_size = self.size(settings.counter) as f32 / total_size as f32;
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
            format!("({})", settings.counter.format(self.size(settings.counter))),
            w_tagline = w_tagline,
        );
        println!(
            "{:55} {}",
            header.color(settings.colorizer.colorize(rel_size)),
            "▒".repeat((rel_size * w_bar as f32) as usize)
        );
        if let Some(children) = &self.children {
            for child in children {
                child.render(total_size, depth + 1, threshold, &settings);
            }
        }
    }

    fn size(&self, count: Unit) -> usize {
        match count {
            Unit::Bytes => self.size_b,
            Unit::Children => self.size_c,
        }
    }

    fn max_depth(&self) -> usize {
        fn _max_depth(n: &Node, ax: usize) -> usize {
            match n.children {
                Some(ref children) => {
                    1 + children
                        .iter()
                        .map(|c| _max_depth(c, ax))
                        .max()
                        .unwrap_or(0)
                }
                None => ax,
            }
        }

        _max_depth(self, 0)
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

    let settings = DisplaySettings {
        counter: args.unit,
        colorizer: args.colors,
        depth: args.max_depth.map(|d| {
            if d >= 0 {
                d as usize
            } else {
                ((root.max_depth() as isize) + d - 1) as usize
            }
        }),
        width,
    };
    root.render(root.size(args.unit), 0, args.threshold / 100., &settings);

    Ok(())
}
