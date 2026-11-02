//! Throwaway: time build_tree for every preset at maturity and at age 4.
use bevy_flora::{TreePreset, build_tree};
use std::time::Instant;
fn median(mut v: Vec<f64>) -> f64 { v.sort_by(|a, b| a.partial_cmp(b).unwrap()); v[v.len() / 2] }
fn main() {
    let mut total = [0.0f64; 2];
    for preset in TreePreset::ALL {
        let p = preset.params();
        let mut row = format!("{:14}", preset.name());
        for (k, age) in [f32::INFINITY, 4.0].into_iter().enumerate() {
            let mut times = Vec::new();
            let mut verts = 0;
            let mut tris = 0;
            for _ in 0..30 {
                let t0 = Instant::now();
                let t = build_tree(&p, age);
                times.push(t0.elapsed().as_secs_f64() * 1e6);
                verts = t.trunk.positions.len() + t.foliage.positions.len() + t.flowers.positions.len();
                tris = (t.trunk.indices.len() + t.foliage.indices.len() + t.flowers.indices.len()) / 3;
            }
            let m = median(times);
            total[k] += m;
            row += &format!("  age {:>4}: {:8.0} us {:7} v {:7} t", if age.is_finite() { "4" } else { "inf" }, m, verts, tris);
        }
        println!("{row}");
    }
    println!("TOTAL inf {:.0} us, age4 {:.0} us", total[0], total[1]);
}
