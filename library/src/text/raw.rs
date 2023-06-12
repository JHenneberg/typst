use once_cell::sync::Lazy;
use syntect::highlighting as synt;
use typst::syntax::{self, LinkedNode};

use super::{
    FontFamily, FontList, Hyphenate, LinebreakElem, SmartQuoteElem, TextElem, TextSize,
};
use crate::layout::BlockElem;
use crate::meta::{Figurable, LocalName};
use crate::prelude::*;

/// Raw text with optional syntax highlighting.
///
/// Displays the text verbatim and in a monospace font. This is typically used
/// to embed computer code into your document.
///
/// ## Example { #example }
/// ````example
/// Adding `rbx` to `rcx` gives
/// the desired result.
///
/// ```rust
/// fn main() {
///     println!("Hello World!");
/// }
/// ```
/// ````
///
/// ## Syntax { #syntax }
/// This function also has dedicated syntax. You can enclose text in 1 or 3+
/// backticks (`` ` ``) to make it raw. Two backticks produce empty raw text.
/// When you use three or more backticks, you can additionally specify a
/// language tag for syntax highlighting directly after the opening backticks.
/// Within raw blocks, everything is rendered as is, in particular, there are no
/// escape sequences.
///
/// Display: Raw Text / Code
/// Category: text
#[element(Synthesize, Show, Finalize, LocalName, Figurable, PlainText)]
pub struct RawElem {
    /// The raw text.
    ///
    /// You can also use raw blocks creatively to create custom syntaxes for
    /// your automations.
    ///
    /// ````example
    /// // Parse numbers in raw blocks with the
    /// // `mydsl` tag and sum them up.
    /// #show raw.where(lang: "mydsl"): it => {
    ///   let sum = 0
    ///   for part in it.text.split("+") {
    ///     sum += int(part.trim())
    ///   }
    ///   sum
    /// }
    ///
    /// ```mydsl
    /// 1 + 2 + 3 + 4 + 5
    /// ```
    /// ````
    #[required]
    pub text: EcoString,

    /// Whether the raw text is displayed as a separate block.
    ///
    /// In markup mode, using one-backtick notation makes this `{false}`,
    /// whereas using three-backtick notation makes it `{true}`.
    ///
    /// ````example
    /// // Display inline code in a small box
    /// // that retains the correct baseline.
    /// #show raw.where(block: false): box.with(
    ///   fill: luma(240),
    ///   inset: (x: 3pt, y: 0pt),
    ///   outset: (y: 3pt),
    ///   radius: 2pt,
    /// )
    ///
    /// // Display block code in a larger block
    /// // with more padding.
    /// #show raw.where(block: true): block.with(
    ///   fill: luma(240),
    ///   inset: 10pt,
    ///   radius: 4pt,
    /// )
    ///
    /// With `rg`, you can search through your files quickly.
    ///
    /// ```bash
    /// rg "Hello World"
    /// ```
    /// ````
    #[default(false)]
    pub block: bool,

    /// The language to syntax-highlight in.
    ///
    /// Apart from typical language tags known from Markdown, this supports the
    /// `{"typ"}` and `{"typc"}` tags for Typst markup and Typst code,
    /// respectively.
    ///
    /// ````example
    /// ```typ
    /// This is *Typst!*
    /// ```
    /// ````
    pub lang: Option<EcoString>,

    /// The horizontal alignment that each line in a raw block should have.
    /// This option is ignored if this is not a raw block (if specified
    /// `block: false` or single backticks were used in markup mode).
    ///
    /// By default, this is set to `{start}`, meaning that raw text is
    /// aligned towards the start of the text direction inside the block
    /// by default, regardless of the current context's alignment (allowing
    /// you to center the raw block itself without centering the text inside
    /// it, for example).
    ///
    /// ````example
    /// #set raw(align: center)
    ///
    /// ```typc
    /// let f(x) = x
    /// code = "centered"
    /// ```
    /// ````
    #[default(HorizontalAlign(GenAlign::Start))]
    pub align: HorizontalAlign,
}

impl RawElem {
    /// The supported language names and tags.
    pub fn languages() -> Vec<(&'static str, Vec<&'static str>)> {
        SYNTAXES
            .syntaxes()
            .iter()
            .map(|syntax| {
                (
                    syntax.name.as_str(),
                    syntax.file_extensions.iter().map(|s| s.as_str()).collect(),
                )
            })
            .chain([("Typst", vec!["typ"]), ("Typst (code)", vec!["typc"])])
            .collect()
    }
}

impl Synthesize for RawElem {
    fn synthesize(&mut self, _vt: &mut Vt, styles: StyleChain) -> SourceResult<()> {
        self.push_lang(self.lang(styles));
        Ok(())
    }
}

impl Show for RawElem {
    #[tracing::instrument(name = "RawElem::show", skip_all)]
    fn show(&self, _: &mut Vt, styles: StyleChain) -> SourceResult<Content> {
        let text = self.text();
        let lang = self.lang(styles).as_ref().map(|s| s.to_lowercase());
        let foreground = THEME
            .settings
            .foreground
            .map(to_typst)
            .map_or(Color::BLACK, Color::from);

        let mut realized = if matches!(lang.as_deref(), Some("typ" | "typst" | "typc")) {
            let root = match lang.as_deref() {
                Some("typc") => syntax::parse_code(&text),
                _ => syntax::parse(&text),
            };

            let mut seq = vec![];
            let highlighter = synt::Highlighter::new(&THEME);
            highlight_themed(
                &LinkedNode::new(&root),
                vec![],
                &highlighter,
                &mut |node, style| {
                    seq.push(styled(&text[node.range()], foreground.into(), style));
                },
            );

            Content::sequence(seq)
        } else if let Some(syntax) =
            lang.and_then(|token| SYNTAXES.find_syntax_by_token(&token))
        {
            let mut seq = vec![];
            let mut highlighter = syntect::easy::HighlightLines::new(syntax, &THEME);
            for (i, line) in text.lines().enumerate() {
                if i != 0 {
                    seq.push(LinebreakElem::new().pack());
                }

                for (style, piece) in
                    highlighter.highlight_line(line, &SYNTAXES).into_iter().flatten()
                {
                    seq.push(styled(piece, foreground.into(), style));
                }
            }

            Content::sequence(seq)
        } else {
            TextElem::packed(text)
        };

        if self.block(styles) {
            // Align the text before inserting it into the block.
            realized = realized.aligned(Axes::with_x(Some(self.align(styles).into())));
            realized = BlockElem::new().with_body(Some(realized)).pack();
        }

        Ok(realized)
    }
}

impl Finalize for RawElem {
    fn finalize(&self, realized: Content, _: StyleChain) -> Content {
        let mut styles = Styles::new();
        styles.set(TextElem::set_overhang(false));
        styles.set(TextElem::set_hyphenate(Hyphenate(Smart::Custom(false))));
        styles.set(TextElem::set_size(TextSize(Em::new(0.8).into())));
        styles
            .set(TextElem::set_font(FontList(vec![FontFamily::new("DejaVu Sans Mono")])));
        styles.set(SmartQuoteElem::set_enabled(false));
        realized.styled_with_map(styles)
    }
}

impl LocalName for RawElem {
    fn local_name(&self, lang: Lang, _: Option<Region>) -> &'static str {
        match lang {
            Lang::ARABIC => "قائمة",
            Lang::BOKMÅL => "Utskrift",
            Lang::CHINESE => "代码",
            Lang::CZECH => "Seznam",
            Lang::DANISH => "Liste",
            Lang::DUTCH => "Listing",
            Lang::FRENCH => "Liste",
            Lang::GERMAN => "Listing",
            Lang::ITALIAN => "Codice",
            Lang::NYNORSK => "Utskrift",
            Lang::POLISH => "Program",
            Lang::RUSSIAN => "Листинг",
            Lang::SLOVENIAN => "Program",
            Lang::SWEDISH => "Listing",
            Lang::TURKISH => "Liste",
            Lang::UKRAINIAN => "Лістинг",
            Lang::VIETNAMESE => "Chương trình", // TODO: This may be wrong.
            Lang::ENGLISH | _ => "Listing",
        }
    }
}

impl Figurable for RawElem {}

impl PlainText for RawElem {
    fn plain_text(&self, text: &mut EcoString) {
        text.push_str(&self.text());
    }
}

/// Highlight a syntax node in a theme by calling `f` with ranges and their
/// styles.
fn highlight_themed<F>(
    node: &LinkedNode,
    scopes: Vec<syntect::parsing::Scope>,
    highlighter: &synt::Highlighter,
    f: &mut F,
) where
    F: FnMut(&LinkedNode, synt::Style),
{
    if node.children().len() == 0 {
        let style = highlighter.style_for_stack(&scopes);
        f(node, style);
        return;
    }

    for child in node.children() {
        let mut scopes = scopes.clone();
        if let Some(tag) = typst::ide::highlight(&child) {
            scopes.push(syntect::parsing::Scope::new(tag.tm_scope()).unwrap())
        }
        highlight_themed(&child, scopes, highlighter, f);
    }
}

/// Style a piece of text with a syntect style.
fn styled(piece: &str, foreground: Paint, style: synt::Style) -> Content {
    let mut body = TextElem::packed(piece);

    let paint = to_typst(style.foreground).into();
    if paint != foreground {
        body = body.styled(TextElem::set_fill(paint));
    }

    if style.font_style.contains(synt::FontStyle::BOLD) {
        body = body.strong();
    }

    if style.font_style.contains(synt::FontStyle::ITALIC) {
        body = body.emph();
    }

    if style.font_style.contains(synt::FontStyle::UNDERLINE) {
        body = body.underlined();
    }

    body
}

fn to_typst(synt::Color { r, g, b, a }: synt::Color) -> RgbaColor {
    RgbaColor { r, g, b, a }
}

fn to_syn(RgbaColor { r, g, b, a }: RgbaColor) -> synt::Color {
    synt::Color { r, g, b, a }
}

/// The syntect syntax definitions.
///
/// Code for syntax set generation is below. The `syntaxes` directory is from
/// <https://github.com/sharkdp/bat/tree/master/assets/syntaxes>
///
/// ```ignore
/// fn main() {
///     let mut builder = syntect::parsing::SyntaxSet::load_defaults_nonewlines().into_builder();
///     builder.add_from_folder("syntaxes/02_Extra", false).unwrap();
///     syntect::dumps::dump_to_file(&builder.build(), "syntect.bin").unwrap();
/// }
/// ```
///
/// The following syntaxes are disabled due to compatibility issues:
/// ```text
/// syntaxes/02_Extra/Assembly (ARM).sublime-syntax
/// syntaxes/02_Extra/Elixir/Regular Expressions (Elixir).sublime-syntax
/// syntaxes/02_Extra/JavaScript (Babel).sublime-syntax
/// syntaxes/02_Extra/LiveScript.sublime-syntax
/// syntaxes/02_Extra/PowerShell.sublime-syntax
/// syntaxes/02_Extra/SCSS_Sass/Syntaxes/Sass.sublime-syntax
/// syntaxes/02_Extra/SLS/SLS.sublime-syntax
/// syntaxes/02_Extra/VimHelp.sublime-syntax
/// syntaxes/02_Extra/cmd-help/syntaxes/cmd-help.sublime-syntax
/// ```
static SYNTAXES: Lazy<syntect::parsing::SyntaxSet> = Lazy::new(|| {
    syntect::dumps::from_binary(include_bytes!("../../../assets/data/syntect.bin"))
});

/// The default theme used for syntax highlighting.
pub static THEME: Lazy<synt::Theme> = Lazy::new(|| synt::Theme {
    name: Some("Typst Light".into()),
    author: Some("The Typst Project Developers".into()),
    settings: synt::ThemeSettings::default(),
    scopes: vec![
        item("comment", Some("#8a8a8a"), None),
        item("constant.character.escape", Some("#1d6c76"), None),
        item("markup.bold", None, Some(synt::FontStyle::BOLD)),
        item("markup.italic", None, Some(synt::FontStyle::ITALIC)),
        item("markup.underline", None, Some(synt::FontStyle::UNDERLINE)),
        item("markup.raw", Some("#818181"), None),
        item("string.other.math.typst", None, None),
        item("punctuation.definition.math", Some("#298e0d"), None),
        item("keyword.operator.math", Some("#1d6c76"), None),
        item("markup.heading, entity.name.section", None, Some(synt::FontStyle::BOLD)),
        item(
            "markup.heading.typst",
            None,
            Some(synt::FontStyle::BOLD | synt::FontStyle::UNDERLINE),
        ),
        item("punctuation.definition.list", Some("#8b41b1"), None),
        item("markup.list.term", None, Some(synt::FontStyle::BOLD)),
        item("entity.name.label, markup.other.reference", Some("#1d6c76"), None),
        item("keyword, constant.language, variable.language", Some("#d73a49"), None),
        item("storage.type, storage.modifier", Some("#d73a49"), None),
        item("constant", Some("#b60157"), None),
        item("string", Some("#298e0d"), None),
        item("entity.name, variable.function, support", Some("#4b69c6"), None),
        item("support.macro", Some("#16718d"), None),
        item("meta.annotation", Some("#301414"), None),
        item("entity.other, meta.interpolation", Some("#8b41b1"), None),
    ],
});

/// Create a syntect theme item.
fn item(
    scope: &str,
    color: Option<&str>,
    font_style: Option<synt::FontStyle>,
) -> synt::ThemeItem {
    synt::ThemeItem {
        scope: scope.parse().unwrap(),
        style: synt::StyleModifier {
            foreground: color.map(|s| to_syn(s.parse::<RgbaColor>().unwrap())),
            background: None,
            font_style,
        },
    }
}
