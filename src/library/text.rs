use std::convert::TryFrom;

use crate::exec::{FontState, LineState};
use crate::font::{FontStretch, FontStyle, FontWeight};
use crate::layout::Paint;

use super::*;

/// `font`: Configure the font.
pub fn font(ctx: &mut EvalContext, args: &mut FuncArgs) -> Value {
    let families: Vec<_> = args.all().collect();
    let list = if families.is_empty() {
        args.named(ctx, "family")
    } else {
        Some(FontDef(families))
    };

    let size = args.eat().or_else(|| args.named::<Linear>(ctx, "size"));
    let style = args.named(ctx, "style");
    let weight = args.named(ctx, "weight");
    let stretch = args.named(ctx, "stretch");
    let top_edge = args.named(ctx, "top-edge");
    let bottom_edge = args.named(ctx, "bottom-edge");
    let fill = args.named(ctx, "fill");
    let serif = args.named(ctx, "serif");
    let sans_serif = args.named(ctx, "sans-serif");
    let monospace = args.named(ctx, "monospace");
    let body = args.expect::<Template>(ctx, "body").unwrap_or_default();

    Value::template(move |ctx| {
        let font = ctx.state.font_mut();

        if let Some(linear) = size {
            font.size = linear.resolve(font.size);
        }

        if let Some(FontDef(list)) = &list {
            font.families_mut().list = list.clone();
        }

        if let Some(style) = style {
            font.variant.style = style;
        }

        if let Some(weight) = weight {
            font.variant.weight = weight;
        }

        if let Some(stretch) = stretch {
            font.variant.stretch = stretch;
        }

        if let Some(top_edge) = top_edge {
            font.top_edge = top_edge;
        }

        if let Some(bottom_edge) = bottom_edge {
            font.bottom_edge = bottom_edge;
        }

        if let Some(fill) = fill {
            font.fill = Paint::Color(fill);
        }

        if let Some(FamilyDef(serif)) = &serif {
            font.families_mut().serif = serif.clone();
        }

        if let Some(FamilyDef(sans_serif)) = &sans_serif {
            font.families_mut().sans_serif = sans_serif.clone();
        }

        if let Some(FamilyDef(monospace)) = &monospace {
            font.families_mut().monospace = monospace.clone();
        }

        body.exec(ctx);
    })
}

#[derive(Debug)]
struct FontDef(Vec<FontFamily>);

castable! {
    FontDef: "font family or array of font families",
    Value::Str(string) => Self(vec![FontFamily::Named(string.to_lowercase())]),
    Value::Array(values) => Self(values
        .into_iter()
        .filter_map(|v| v.cast().ok())
        .collect()
    ),
    #(family: FontFamily) => Self(vec![family]),
}

#[derive(Debug)]
struct FamilyDef(Vec<String>);

castable! {
    FamilyDef: "string or array of strings",
    Value::Str(string) => Self(vec![string.to_lowercase()]),
    Value::Array(values) => Self(values
        .into_iter()
        .filter_map(|v| v.cast().ok())
        .map(|string: EcoString| string.to_lowercase())
        .collect()
    ),
}

castable! {
    FontFamily: "font family",
    Value::Str(string) => Self::Named(string.to_lowercase())
}

castable! {
    FontStyle: "font style",
}

castable! {
    FontWeight: "font weight",
    Value::Int(number) => {
        u16::try_from(number).map_or(Self::BLACK, Self::from_number)
    }
}

castable! {
    FontStretch: "font stretch",
    Value::Relative(relative) => Self::from_ratio(relative.get() as f32),
}

castable! {
    VerticalFontMetric: "vertical font metric",
}

/// `par`: Configure paragraphs.
pub fn par(ctx: &mut EvalContext, args: &mut FuncArgs) -> Value {
    let spacing = args.named(ctx, "spacing");
    let leading = args.named(ctx, "leading");
    let word_spacing = args.named(ctx, "word-spacing");
    let body = args.expect::<Template>(ctx, "body").unwrap_or_default();

    Value::template(move |ctx| {
        if let Some(spacing) = spacing {
            ctx.state.par.spacing = spacing;
        }

        if let Some(leading) = leading {
            ctx.state.par.leading = leading;
        }

        if let Some(word_spacing) = word_spacing {
            ctx.state.par.word_spacing = word_spacing;
        }

        ctx.parbreak();
        body.exec(ctx);
    })
}

/// `lang`: Configure the language.
pub fn lang(ctx: &mut EvalContext, args: &mut FuncArgs) -> Value {
    let iso = args.eat::<EcoString>().map(|s| lang_dir(&s));
    let dir = match args.named::<Spanned<Dir>>(ctx, "dir") {
        Some(dir) if dir.v.axis() == SpecAxis::Horizontal => Some(dir.v),
        Some(dir) => {
            ctx.diag(error!(dir.span, "must be horizontal"));
            None
        }
        None => None,
    };
    let body = args.expect::<Template>(ctx, "body").unwrap_or_default();

    Value::template(move |ctx| {
        if let Some(dir) = dir.or(iso) {
            ctx.state.lang.dir = dir;
        }

        ctx.parbreak();
        body.exec(ctx);
    })
}

/// The default direction for the language identified by `iso`.
fn lang_dir(iso: &str) -> Dir {
    match iso.to_ascii_lowercase().as_str() {
        "ar" | "he" | "fa" | "ur" | "ps" | "yi" => Dir::RTL,
        "en" | "fr" | "de" => Dir::LTR,
        _ => Dir::LTR,
    }
}

/// `strike`: Enable striken-through text.
pub fn strike(ctx: &mut EvalContext, args: &mut FuncArgs) -> Value {
    line_impl(ctx, args, |font| &mut font.strikethrough)
}

/// `underline`: Enable underlined text.
pub fn underline(ctx: &mut EvalContext, args: &mut FuncArgs) -> Value {
    line_impl(ctx, args, |font| &mut font.underline)
}

/// `overline`: Add an overline above text.
pub fn overline(ctx: &mut EvalContext, args: &mut FuncArgs) -> Value {
    line_impl(ctx, args, |font| &mut font.overline)
}

fn line_impl(
    ctx: &mut EvalContext,
    args: &mut FuncArgs,
    substate: fn(&mut FontState) -> &mut Option<Rc<LineState>>,
) -> Value {
    let stroke = args.eat().or_else(|| args.named(ctx, "stroke"));
    let thickness = args.eat().or_else(|| args.named::<Linear>(ctx, "thickness"));
    let offset = args.named(ctx, "offset");
    let extent = args.named(ctx, "extent").unwrap_or_default();
    let body = args.expect::<Template>(ctx, "body").unwrap_or_default();

    // Suppress any existing strikethrough if strength is explicitly zero.
    let state = thickness.map_or(true, |s| !s.is_zero()).then(|| {
        Rc::new(LineState {
            stroke: stroke.map(Paint::Color),
            thickness,
            offset,
            extent,
        })
    });

    Value::template(move |ctx| {
        *substate(ctx.state.font_mut()) = state.clone();
        body.exec(ctx);
    })
}