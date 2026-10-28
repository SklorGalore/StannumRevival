//! A quick picture of a generated network, in the style of the paper's
//! Fig. 5: substations as dots, lines coloured by voltage level.

use crate::Synthetic;

const COLOURS: [&str; 5] = ["#8a94a0", "#2b7bb9", "#d7301f", "#7b3294", "#1a9850"];

pub fn draw(s: &Synthetic, width: f64) -> String {
    let subs = &s.placement.substations;
    let xy: Vec<[f64; 2]> = subs.iter().map(|x| x.xy).collect();
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in &xy {
        x0 = x0.min(p[0]);
        x1 = x1.max(p[0]);
        y0 = y0.min(p[1]);
        y1 = y1.max(p[1]);
    }
    let pad = 20.0;
    let scale = (width - 2.0 * pad) / (x1 - x0).max(1e-9);
    let height = (y1 - y0) * scale + 2.0 * pad + 30.0;
    let at = |p: [f64; 2]| (pad + (p[0] - x0) * scale, pad + (y1 - p[1]) * scale);
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.0}\" height=\"{height:.0}\" viewBox=\"0 0 {width:.0} {height:.0}\">\n\
         <rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>\n"
    );
    let thin = (width / 1600.0).max(0.4);
    for level in 0..s.config.levels.len() {
        let colour = COLOURS[level.min(COLOURS.len() - 1)];
        let w = thin * (1.0 + 0.8 * level as f64);
        out.push_str(&format!("<g stroke=\"{colour}\" stroke-width=\"{w:.2}\" stroke-linecap=\"round\" fill=\"none\">\n"));
        for l in s.lines.iter().filter(|l| l.level == level) {
            let (a, b) = (at(xy[l.a]), at(xy[l.b]));
            out.push_str(&format!(
                "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"/>\n",
                a.0, a.1, b.0, b.1
            ));
        }
        out.push_str("</g>\n");
    }
    let r = (thin * 1.6).max(0.8);
    out.push_str("<g fill=\"#1b2422\">\n");
    for (i, p) in xy.iter().enumerate() {
        let (x, y) = at(*p);
        let rr = if subs[i].plants.is_empty() {
            r
        } else {
            r * 1.8
        };
        out.push_str(&format!(
            "<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"{rr:.2}\"/>\n"
        ));
    }
    out.push_str("</g>\n");
    let mut lx = pad;
    let ly = height - 12.0;
    for (i, l) in s.config.levels.iter().enumerate() {
        let colour = COLOURS[i.min(COLOURS.len() - 1)];
        out.push_str(&format!(
            "<line x1=\"{lx:.0}\" y1=\"{ly:.0}\" x2=\"{:.0}\" y2=\"{ly:.0}\" stroke=\"{colour}\" stroke-width=\"3\"/>\
             <text x=\"{:.0}\" y=\"{:.0}\" font-family=\"sans-serif\" font-size=\"13\" fill=\"#1b2422\">{} kV</text>\n",
            lx + 24.0,
            lx + 30.0,
            ly + 4.0,
            l.kv
        ));
        lx += 100.0;
    }
    out.push_str("</svg>\n");
    out
}
