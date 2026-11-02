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
    if which.starts_with("max") {
        p.foliage.enabled = which == "max";
        p.trunk.taper = 4.0; p.trunk.flare = 16.0; p.trunk.randomness = 8.0; p.trunk.root_flare_count = 64;
        p.trunk.root_flare_spread = 16.0; p.trunk.root_flare_height = 1.0; p.trunk.leader_length = 8.0; p.trunk.leader_taper = 4.0;
        p.branches.end = 1.0; p.branches.radius_ratio = 4.0; p.branches.randomness = 8.0; p.branches.sub_branch_scale = 4.0;
        p.branches.apical_dominance = 4.0; p.branches.flatness = 1.0; p.branches.stiffness = 1.0; p.branches.break_chance = 1.0;
        p.branches.crown_influence = 4.0; p.branches.floor_level = 1000.0; p.branches.min_twig_radius = 10.0;
        p.split.probability = 1.0; p.split.position = 1.0; p.split.radius_threshold = 100.0;
        p.mesh.smooth_factor = 1.0; p.mesh.pipe_radius_exponent = 8.0; p.mesh.pipe_radius_min = 10.0;
        if let Ok(v) = std::env::var("ZZ_OFF") {
            for k in v.split(',') {
                match k {
                    "break" => p.branches.break_chance = 0.0,
                    "floor" => p.branches.floor_level = 0.0,
                    "twig" => p.branches.min_twig_radius = 0.005,
                    "scale" => p.branches.sub_branch_scale = 0.5,
                    "split" => p.split.enabled = false,
                    "pipe" => { p.mesh.pipe_radius_exponent = 2.0; p.mesh.pipe_radius_min = 0.02; }
                    "root" => { p.trunk.root_flare_count = 0; }
                    "flare" => p.trunk.flare = 1.0,
                    "taper" => p.trunk.taper = 0.3,
                    "rand" => { p.trunk.randomness = 0.0; p.branches.randomness = 0.2; }
                    "leader" => { p.trunk.leader_length = 0.15; p.trunk.leader_taper = 0.1; }
                    "smooth" => { p.mesh.smooth_iterations = 0; }
                    "segs" => { p.trunk.radial_segments = 8; p.trunk.height_segments = 4; }
                    "collar" => { p.mesh.collar_length = 1.5; }
                    "rootspread" => { p.trunk.root_flare_spread = 0.5; p.trunk.root_flare_height = 0.15; }
                    "adaptive" => { p.mesh.adaptive_resolution = false; }
                    _ => {}
                }
            }
        }
        p.split.enabled = true;
    }
    println!("start");
    std::thread::spawn(|| loop { std::thread::sleep(std::time::Duration::from_millis(1500)); println!("  ... rss {:.0} MB", rss_mb()); });
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
