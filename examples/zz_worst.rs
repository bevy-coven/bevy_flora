//! Throwaway: the worst tree the ranges allow, timed by stage.
use bevy_flora::*;
use std::time::Instant;
fn rss_mb() -> f64 {
    let out = std::process::Command::new("ps").args(["-o", "rss=", "-p", &std::process::id().to_string()]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse::<f64>().unwrap_or(0.0) / 1024.0
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let which = args.get(1).map(String::as_str).unwrap_or("all");
    let mut p = TreePreset::Birch.params();
    p.trunk.height = 500.0;
    p.trunk.radius = 50.0;
    p.trunk.radial_segments = 64;
    p.trunk.height_segments = 256;
    p.branches.density = 1000.0;
    p.branches.length = 32.0;
    p.branches.recursion = 8;
    p.branches.sub_branch_count = 32;
    p.branches.gravity_strength = 16.0;
    p.branches.stiffness = 0.0;
    p.branches.strands = StrandParams { density: 32.0, start: 0.0, length: 16.0, spread: 90.0, clearance: 0.0 };
    p.mesh.smooth_iterations = 32;
    p.mesh.adaptive_resolution = true;
    p.mesh.min_radial_segments = 64;
    p.mesh.max_radial_segments = 64;
    p.mesh.resolution_scale = 1024.0;
    p.mesh.collar_length = 64.0;
    p.foliage.density = 512.0;
    p.foliage.cluster_size = 64;
    p.foliage.leaf_size = 64.0;
    p.foliage.placement = if which == "terminal" { FoliagePlacement::TerminalBranches } else { FoliagePlacement::AllBranches };
    if which == "wood" {
        p.foliage.enabled = false;
    }
    if which == "nosmooth" {
        p.mesh.smooth_iterations = 0;
    }
    let t0 = Instant::now();
    let skeleton = generate_skeleton(&p);
    println!("skeleton: {} nodes in {:.0} ms, rss {:.0} MB", skeleton.nodes.len(), t0.elapsed().as_secs_f64() * 1e3, rss_mb());
    let t0 = Instant::now();
    let tree = build_tree(&p, f32::INFINITY);
    println!(
        "build_tree: {:.0} ms, trunk {} v, foliage {} v, rss {:.0} MB",
        t0.elapsed().as_secs_f64() * 1e3,
        tree.trunk.positions.len(),
        tree.foliage.positions.len(),
        rss_mb()
    );
    let t0 = Instant::now();
    let lods = build_lods(&p, f32::INFINITY, &LodSettings::default());
    println!("build_lods: {:.0} ms, level 2: {} + {} v, rss {:.0} MB", t0.elapsed().as_secs_f64() * 1e3, lods[2].trunk.positions.len(), lods[2].foliage.positions.len(), rss_mb());
}
