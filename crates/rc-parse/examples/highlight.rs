//! Démonstrateur CLI : colorise un fichier via une grammaire inline.
//!
//! Une petite grammaire « source.rcd » (javadoc-like) est embarquée en JSON :
//! commentaires de bloc, chaînes avec échappements, mots-clés, identifiants
//! et règles d'include — ce qui couvre tous les mécanismes de rc-parse
//! (begin/end multi-ligne, captures, repository, parsing incrémental).
//!
//! Usage :
//! ```
//! cargo run -p rc-parse --example highlight -- chemin/vers/fichier.txt
//! ```
//!
//! Sans argument, un document d'exemple est affiché. Chaque plage est
//! colorisée d'après son scope (thème minimaliste, ANSI 256).

use rc_buffer::Buffer;
use rc_parse::{Grammar, Parser};

const GRAMMAR_JSON: &str = r##"
{
  "scopeName": "source.rcd",
  "name": "RightClick Demo",
  "repository": {
    "line-comment": {
      "begin": "//",
      "end": "$",
      "name": "comment.line.double-slash"
    }
  },
  "patterns": [
    { "include": "#line-comment" },
    {
      "begin": "/\\*",
      "end": "\\*/",
      "name": "comment.block",
      "patterns": [ { "include": "$self" } ]
    },
    {
      "begin": "\"",
      "end": "\"",
      "name": "string.quoted.double",
      "patterns": [
        { "match": "\\\\[\"\\\\nrt0]", "name": "constant.character.escape" }
      ]
    },
    {
      "match": "\\b(function|return|if|else|for|while|let|const)\\b",
      "name": "keyword.control"
    },
    {
      "match": "\\b\\d+(\\.\\d+)?\\b",
      "name": "constant.numeric"
    },
    {
      "match": "\\b[A-Za-z_][A-Za-z0-9_]*\\b",
      "name": "variable.other"
    }
  ]
}
"##;

const DEMO: &str = "\
// rc-parse : le moteur de grammaires TextMate, en Rust.
/* Un commentaire de bloc
   peut s'étendre sur plusieurs lignes. */
function greet(name) {
    let msg = \"Salut, \" + name + \" !\";  // \n échappé ?
    if (msg.length > 40) {
        return msg + \"…\";
    } else {
        return \"court\";
    }
}
";

/// Thème minimaliste : scope → couleur ANSI 256.
fn color(scope: &str) -> &'static str {
    if scope.contains("comment") {
        "\x1b[38;5;243m" // gris
    } else if scope.contains("string") {
        "\x1b[38;5;36m" // vert d'eau
    } else if scope.contains("keyword") {
        "\x1b[38;5;202m" // orange
    } else if scope.contains("constant.numeric") {
        "\x1b[38;5;141m" // violet
    } else if scope.contains("constant.character.escape") {
        "\x1b[38;5;196m" // rouge
    } else if scope.contains("variable") {
        "\x1b[38;5;75m" // bleu
    } else {
        "\x1b[0m"
    }
}

fn main() {
    let g = Grammar::from_json_str(GRAMMAR_JSON).expect("grammaire embarquée");
    let mut parser = Parser::from_grammar(g);

    let text = std::env::args()
        .nth(1)
        .map(|p| std::fs::read_to_string(&p).expect("lecture du fichier"))
        .unwrap_or_else(|| DEMO.to_string());
    let buffer = Buffer::from_str(&text);
    let lines = parser.parse_all(&buffer);

    println!("{} lignes analysées ({lines} re-parsées)\n", buffer.line_count());
    let reset = "\x1b[0m";
    for i in 0..buffer.line_count() {
        let chars: Vec<char> = buffer.line(i).chars().collect();
        let mut out = String::new();
        let mut cursor = 0;
        for r in parser.line_ranges(i) {
            if r.range.start > cursor {
                let gap: String = chars[cursor..r.range.start].iter().collect();
                out.push_str(&gap);
            }
            out.push_str(color(&r.scope.to_string()));
            let hit: String = chars[r.range.start..r.range.end].iter().collect();
            out.push_str(&hit);
            out.push_str(reset);
            cursor = r.range.end;
        }
        if cursor < chars.len() {
            let tail: String = chars[cursor..].iter().collect();
            out.push_str(&tail);
        }
        println!("{out}");
    }
}