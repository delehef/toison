use anyhow::*;
use clap::Parser;
use colored::Colorize;
use serde_json::Value;
use thousands::Separable;

const WIDTH: usize = 50;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    json_file: String,

    #[arg(short, long, default_value_t = 0.0)]
    threshold: f32,
}

#[derive(Debug, Clone)]
struct Node {
    tag: Option<String>,
    size: usize,
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
                    tag: Some(format!(
                        "{} (×{})",
                        tag,
                        children.len().separate_with_commas()
                    )),
                    size: children.iter().map(|c| c.size).sum::<usize>(),
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
                    size: children.iter().map(|c| c.size).sum::<usize>(),
                    key_size: _children.keys().map(|k| k.len()).sum::<usize>(),
                    children: Some(children),
                }
            }
        }
    }

    fn leaf(key_size: usize, size: usize, tag: String) -> Node {
        Node {
            tag: if tag.is_empty() { None } else { Some(tag) },
            size,
            key_size,
            children: None,
        }
    }

    fn render(&self, total_size: usize, depth: usize, threshold: f32, pad_tag: usize) {
        let rel_size = self.size as f32 / total_size as f32;
        if rel_size < threshold {
            return;
        }

        let indent = " ".repeat(2 * depth);
        let header = format!(
            "{}{:0width$} {:.2}%",
            indent,
            self.tag.as_ref().cloned().unwrap_or_default(),
            100. * rel_size,
            width = pad_tag,
        );
        println!(
            "{:60}{}",
            header.truecolor(100 + (155 as f32 * rel_size) as u8, 100, 100),
            "#".repeat((rel_size * WIDTH as f32) as usize)
        );
        if let Some(children) = &self.children {
            let pad_tag = children
                .iter()
                .filter(|c| c.size as f32 / total_size as f32 >= threshold)
                .map(|c| c.tag.as_ref().map(|x| x.len()).unwrap_or(0))
                .max()
                .unwrap_or(0);

            for child in children {
                child.render(total_size, depth + 1, threshold, pad_tag);
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

    // dbg!(root);
    root.render(root.size, 0, args.threshold / 100., 0);

    Ok(())
}
