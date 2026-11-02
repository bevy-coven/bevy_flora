//! What a tree may be asked for: the range of every numeric [`TreeParams`] field, and the size no tree
//! exceeds.
//!
//! A [`TreeParams`] comes from callers and from `.tree.ron` files, and an editor hands every field to a
//! human, so any value can arrive: `NaN`, an infinity, a negative height, `1e30`, `u32::MAX`
//! branches. Without a range the generator's cost is unbounded (a trunk 1e6 m tall, a density of 1e6,
//! 255 sub-branches a branch) and its meshes can hold `NaN`. [`TreeParams::sanitized`] is the one
//! place that says what the generator accepts, and the generating functions ([`build_tree`],
//! [`build_lods`], [`generate_skeleton`], [`skeleton_mesh`]) and the plugin call it, so a tree is always
//! drawn from parameters inside the ranges below. [`MAX_NODES`] and [`MAX_LEAVES`] bound what ranges
//! that are each fine can multiply to.
//!
//! Pure CPU: this module reads only [`crate::params`].
//!
//! [`build_tree`]: crate::build_tree
//! [`build_lods`]: crate::build_lods
//! [`generate_skeleton`]: crate::generate_skeleton
//! [`skeleton_mesh`]: crate::mesh::branch::skeleton_mesh

use std::borrow::Cow;
use std::sync::LazyLock;

use bevy::color::Color;

use crate::params::{CrownStyle, FruitParams, TreeParams, TrunkStyle};

/// The most branches (nodes of the [`Skeleton`](crate::Skeleton): trunk, leader, primaries, splits,
/// sub-branches and strands) a tree has. Generation stops adding wood here, in its own order (the
/// leader, then each primary with its splits and sub-branches, then the strands), so a tree that asks for
/// more keeps its lower and earlier wood. The 20 presets have 5 to 588 (the largest gallery scenario 852),
/// so nothing the crate draws comes near it.
pub const MAX_NODES: usize = 5_000;

/// The most leaves a tree's skeleton can carry: when the leaves its nodes could bear
/// ([`FoliageParams`](crate::FoliageParams): terminal count, cluster size or density along the wood)
/// add up to more, the last nodes are dropped until they fit. The heaviest preset (the willow, with its
/// strands) has 4 630.
pub const MAX_LEAVES: usize = 20_000;

/// The most leaves one node of a tree grown under
/// [`FoliagePlacement::AllBranches`](crate::FoliagePlacement::AllBranches) carries however long it is.
pub(crate) const MAX_LEAVES_PER_NODE: f32 = 1024.0;

/// `value` inside `lo..=hi`, or `default` (also inside it) when it is `NaN`. `f32::clamp` alone returns
/// `NaN` for `NaN`.
fn clean(value: f32, default: f32, lo: f32, hi: f32) -> f32 {
    if value.is_nan() { default.clamp(lo, hi) } else { value.clamp(lo, hi) }
}

/// Whether `color` has four finite channels.
fn finite(color: Color) -> bool {
    let c = color.to_linear();
    c.red.is_finite() && c.green.is_finite() && c.blue.is_finite() && c.alpha.is_finite()
}

/// Applies the table below to a copy of the parameters and notes whether anything moved.
struct Limits {
    changed: bool,
}

impl Limits {
    /// A float field: `NaN` reads as `default`, anything else is held in `lo..=hi`. `-0.0` is left alone.
    fn float(&mut self, field: &mut f32, default: f32, lo: f32, hi: f32) {
        let new = clean(*field, default, lo, hi);
        // `NaN != x` is true, so a `NaN` always counts as moved; `-0.0 == 0.0`, so it never does.
        if new != *field {
            *field = new;
            self.changed = true;
        }
    }

    /// A count: held in `lo..=hi`.
    fn count(&mut self, field: &mut u32, lo: u32, hi: u32) {
        let new = (*field).clamp(lo, hi);
        if new != *field {
            *field = new;
            self.changed = true;
        }
    }

    /// A colour with a `NaN` or infinite channel reads as `default`.
    fn color(&mut self, field: &mut Color, default: Color) {
        if !finite(*field) {
            *field = default;
            self.changed = true;
        }
    }

    /// An optional colour with a `NaN` or infinite channel is `None`, the default of every one.
    fn optional_color(&mut self, field: &mut Option<Color>) {
        if field.is_some_and(|color| !finite(color)) {
            *field = None;
            self.changed = true;
        }
    }
}

static DEFAULTS: LazyLock<TreeParams> = LazyLock::new(TreeParams::default);

impl TreeParams {
    /// The parameters a tree is actually built from: these, with every numeric field inside the range the
    /// generator is defined over. Borrowed when nothing needs to move (every preset, and every value
    /// the gallery draws); otherwise a copy where a `NaN` reads as the field's [`Default`] and any other
    /// value that is out of range, an infinity included, is the nearest end of its range.
    ///
    /// [`build_tree`](crate::build_tree), [`build_lods`](crate::build_lods),
    /// [`generate_skeleton`](crate::generate_skeleton), [`skeleton_mesh`](crate::mesh::branch::skeleton_mesh)
    /// and [`TreeParams::mature_at`] call it on what they are given, and [`FloraPlugin`](crate::FloraPlugin)
    /// writes it back into a [`ProceduralTree`](crate::ProceduralTree) that is added or changed, so a
    /// tree is built in bounded time from finite values whatever a `.tree.ron` file or an editor's number
    /// field held. An editor that shows the value in force asks for it here.
    ///
    /// The ranges are wide, and every one has more than a factor of ten of room over the presets and the
    /// gallery's extreme scenarios: heights and lengths in metres up to 500 and 32, fractions of a height
    /// from 0 to 1, angles to a few turns, `branches.recursion` to 8, `sub_branch_count` to 32,
    /// `radial_segments` to 64, `smooth_iterations` to 8, leaf `density` to 512. Enums, flags and the
    /// seed are not limited, and a colour with a `NaN` channel reads as its default. A tree that
    /// the ranges allow can still ask for more wood or leaves than any tree has: [`MAX_NODES`] and
    /// [`MAX_LEAVES`] cut it.
    ///
    /// ```
    /// use bevy_flora::{TreeParams, TreePreset, build_tree};
    ///
    /// // A preset is already inside every range.
    /// assert!(matches!(TreePreset::Oak.params().sanitized(), std::borrow::Cow::Borrowed(_)));
    ///
    /// // A number field that held `NaN`, and one that held 1e9 metres, draw the default height and the tallest.
    /// let mut params = TreeParams::default();
    /// params.trunk.height = f32::NAN;
    /// assert_eq!(params.sanitized().trunk.height, TreeParams::default().trunk.height);
    /// params.trunk.height = 1e9;
    /// assert_eq!(params.sanitized().trunk.height, 500.0);
    /// assert!(build_tree(&params, f32::INFINITY).trunk.positions.iter().all(|p| p.is_finite()));
    /// ```
    pub fn sanitized(&self) -> Cow<'_, TreeParams> {
        let mut copy = self.clone();
        if copy.apply_limits() { Cow::Owned(copy) } else { Cow::Borrowed(self) }
    }

    /// Hold every numeric field in its range; whether any moved.
    fn apply_limits(&mut self) -> bool {
        let d = &*DEFAULTS;
        let p = self;
        let mut l = Limits { changed: false };

        // Trunk: metres, fractions of its height, counts and degrees.
        l.float(&mut p.trunk.height, d.trunk.height, 0.0, 500.0);
        l.float(&mut p.trunk.radius, d.trunk.radius, 0.0, 50.0);
        l.count(&mut p.trunk.radial_segments, 3, 64);
        l.count(&mut p.trunk.height_segments, 1, 256);
        l.float(&mut p.trunk.taper, d.trunk.taper, 0.0, 4.0);
        // An exponent: `t^-1` is infinite at the base.
        l.float(&mut p.trunk.taper_curve, d.trunk.taper_curve, 0.0, 16.0);
        l.float(&mut p.trunk.flare, d.trunk.flare, 0.0, 16.0);
        l.float(&mut p.trunk.randomness, d.trunk.randomness, 0.0, 8.0);
        l.count(&mut p.trunk.root_flare_count, 0, 64);
        l.float(&mut p.trunk.root_flare_spread, d.trunk.root_flare_spread, 0.0, 16.0);
        l.float(&mut p.trunk.root_flare_height, d.trunk.root_flare_height, 0.0, 1.0);
        l.float(&mut p.trunk.leader_length, d.trunk.leader_length, 0.0, 8.0);
        l.float(&mut p.trunk.leader_taper, d.trunk.leader_taper, 0.0, 4.0);
        l.float(&mut p.trunk.twist, d.trunk.twist, -7200.0, 7200.0);

        // Branches. The zone is a fraction of the trunk's height: below 0 is wood under the ground.
        let b = &mut p.branches;
        let db = &d.branches;
        l.float(&mut b.start, db.start, 0.0, 1.0);
        l.float(&mut b.end, db.end, 0.0, 1.0);
        // Per metre of trunk: the fern's 6 cm stem carries 300. The nodes are capped, not the density.
        l.float(&mut b.density, db.density, 0.0, 1000.0);
        l.float(&mut b.length, db.length, 0.0, 32.0);
        l.float(&mut b.angle, db.angle, -360.0, 360.0);
        l.float(&mut b.radius_ratio, db.radius_ratio, 0.0, 4.0);
        // A tip radius is `base * (1 - taper)`: above 1 is a negative radius.
        l.float(&mut b.taper, db.taper, 0.0, 1.0);
        l.float(&mut b.phyllotaxis_angle, db.phyllotaxis_angle, -3600.0, 3600.0);
        l.float(&mut b.randomness, db.randomness, 0.0, 8.0);
        l.float(&mut b.up_attraction, db.up_attraction, -8.0, 8.0);
        l.count(&mut b.recursion, 0, 8);
        l.count(&mut b.sub_branch_count, 0, 32);
        l.float(&mut b.sub_branch_scale, db.sub_branch_scale, 0.0, 4.0);
        // A length is scaled by `1 - variation` at the least.
        l.float(&mut b.length_variation, db.length_variation, 0.0, 1.0);
        l.float(&mut b.sub_branch_position_bias, db.sub_branch_position_bias, -1.0, 1.0);
        l.float(&mut b.apical_dominance, db.apical_dominance, 0.0, 4.0);
        l.float(&mut b.flatness, db.flatness, 0.0, 1.0);
        l.float(&mut b.angle_curve, db.angle_curve, -8.0, 8.0);
        l.float(&mut b.crown_angle_variation, db.crown_angle_variation, -8.0, 8.0);
        l.float(&mut b.twist, db.twist, -7200.0, 7200.0);
        l.float(&mut b.gravity_strength, db.gravity_strength, -16.0, 16.0);
        // Above 1 reads as 1 (`bend = gravity * (1 - stiffness)`, floored at 0).
        l.float(&mut b.stiffness, db.stiffness, 0.0, 1.0);
        l.float(&mut b.break_chance, db.break_chance, 0.0, 1.0);
        l.float(&mut b.crown_influence, db.crown_influence, 0.0, 4.0);
        l.float(&mut b.floor_level, db.floor_level, -1000.0, 1000.0);
        l.float(&mut b.min_twig_radius, db.min_twig_radius, 0.0, 10.0);
        let s = &mut b.strands;
        let ds = &db.strands;
        l.float(&mut s.density, ds.density, 0.0, 32.0);
        l.float(&mut s.start, ds.start, 0.0, 1.0);
        l.float(&mut s.length, ds.length, 0.0, 16.0);
        l.float(&mut s.spread, ds.spread, 0.0, 90.0);
        l.float(&mut s.clearance, ds.clearance, -100.0, 100.0);

        let s = &mut p.split;
        let ds = &d.split;
        l.float(&mut s.probability, ds.probability, 0.0, 1.0);
        l.float(&mut s.angle, ds.angle, 0.0, 360.0);
        l.float(&mut s.position, ds.position, 0.0, 1.0);
        l.float(&mut s.radius_threshold, ds.radius_threshold, 0.0, 100.0);

        // Mesh construction: the counts bound the vertex cost of the wood.
        let m = &mut p.mesh;
        let dm = &d.mesh;
        l.count(&mut m.smooth_iterations, 0, 8);
        l.float(&mut m.smooth_factor, dm.smooth_factor, 0.0, 1.0);
        l.float(&mut m.resolution_scale, dm.resolution_scale, 0.0, 1024.0);
        // The two bounds may cross (the upper one wins), so each has the whole range.
        l.count(&mut m.min_radial_segments, 0, 32);
        l.count(&mut m.max_radial_segments, 0, 32);
        l.float(&mut m.pipe_radius_exponent, dm.pipe_radius_exponent, 0.5, 8.0);
        l.float(&mut m.pipe_radius_min, dm.pipe_radius_min, 0.0, 10.0);
        l.float(&mut m.collar_length, dm.collar_length, 0.0, 64.0);

        // Leaves. Density is per branch, or per metre of wood: with the height falloff at most 2x.
        let f = &mut p.foliage;
        let df = &d.foliage;
        l.float(&mut f.density, df.density, 0.0, 512.0);
        l.count(&mut f.cluster_size, 0, 64);
        l.float(&mut f.leaf_size, df.leaf_size, 0.0, 64.0);
        l.float(&mut f.leaf_size_variation, df.leaf_size_variation, 0.0, 1.0);
        l.float(&mut f.radius_threshold, df.radius_threshold, 0.0, 100.0);
        // The density at the crown's base is `1 - falloff` times its top's: 0 to 2.
        l.float(&mut f.height_falloff, df.height_falloff, -1.0, 1.0);
        l.float(&mut f.droop, df.droop, -8.0, 8.0);
        l.float(&mut f.rotation_variation, df.rotation_variation, 0.0, 1.0);
        l.float(&mut f.normal_bend, df.normal_bend, 0.0, 1.0);
        l.float(&mut f.color_variation, df.color_variation, 0.0, 1.0);
        l.float(&mut f.hue_variation, df.hue_variation, 0.0, 1.0);
        l.float(&mut f.occlusion, df.occlusion, 0.0, 1.0);
        let v = &mut f.shading;
        let dv = &df.shading;
        l.float(&mut v.translucency, dv.translucency, 0.0, 16.0);
        l.float(&mut v.roughness, dv.roughness, 0.0, 1.0);
        l.float(&mut v.veins, dv.veins, 0.0, 1.0);
        l.float(&mut v.underside, dv.underside, 0.0, 1.0);
        l.color(&mut v.transmission_tint, dv.transmission_tint);
        l.color(&mut f.color, df.color);
        l.optional_color(&mut f.autumn);
        l.optional_color(&mut f.blossom);
        if let Some(fruit) = &mut f.fruit {
            let dfruit = FruitParams::default();
            l.float(&mut fruit.radius, dfruit.radius, 0.0, 4.0);
            l.float(&mut fruit.share, dfruit.share, 0.0, 1.0);
            l.color(&mut fruit.color, dfruit.color);
        }

        let w = &mut p.wind;
        let dw = &d.wind;
        l.float(&mut w.sway, dw.sway, 0.0, 64.0);
        l.float(&mut w.flutter, dw.flutter, 0.0, 64.0);
        l.float(&mut w.sway_frequency, dw.sway_frequency, 0.0, 64.0);
        l.float(&mut w.flutter_frequency, dw.flutter_frequency, 0.0, 64.0);

        l.float(&mut p.bark.smooth_below, d.bark.smooth_below, 0.0, 10.0);
        l.float(&mut p.bark.fissured_above, d.bark.fissured_above, 0.0, 10.0);
        l.color(&mut p.trunk_color, d.trunk_color);

        let g = &mut p.growth;
        let dg = &d.growth;
        l.float(&mut g.maturity_age, dg.maturity_age, 0.0, 10_000.0);
        l.float(&mut g.trunk_fraction, dg.trunk_fraction, 0.0, 8.0);
        l.float(&mut g.branch_fraction, dg.branch_fraction, 0.0, 8.0);
        l.float(&mut g.child_duration_scale, dg.child_duration_scale, 0.0, 8.0);
        l.float(&mut g.thickening_lag, dg.thickening_lag, 0.0, 64.0);
        l.float(&mut g.leaf_threshold, dg.leaf_threshold, 0.0, 1.0);

        if let CrownStyle::Hull { tiers, sides, lift, jitter, .. } = &mut p.crown {
            l.count(tiers, 1, 64);
            l.count(sides, 3, 64);
            l.float(lift, 0.0, 0.0, 1.0);
            l.float(jitter, 0.0, 0.0, 1.0);
        }
        if let TrunkStyle::Prism { sides } = &mut p.trunk_style {
            l.count(sides, 3, 64);
        }
        l.changed
    }
}

#[cfg(test)]
pub(crate) mod fields {
    //! Every numeric field of a [`TreeParams`](crate::TreeParams), found by reflection, so a test that
    //! sets each one covers a field added later without anyone remembering it.

    use bevy::reflect::{PartialReflect, ReflectMut};

    /// The name of the child of `value` at `index`, for a path: a struct's field name, an enum's variant
    /// field name, or the position of a tuple field.
    fn child<'a>(value: &'a mut dyn PartialReflect, name: &str) -> Option<&'a mut dyn PartialReflect> {
        match value.reflect_mut() {
            ReflectMut::Struct(s) => s.field_mut(name),
            ReflectMut::Enum(e) => {
                let index = (0..e.field_len()).find(|&i| e.name_at(i).map_or_else(|| i.to_string(), str::to_string) == name)?;
                e.field_at_mut(index)
            }
            ReflectMut::Tuple(t) => t.field_mut(name.parse().ok()?),
            _ => None,
        }
    }

    /// Paths (`.trunk.height`, `.crown.Hull.tiers`) of every `f32` and `u32` under `value`.
    pub(crate) fn numeric(value: &mut dyn PartialReflect) -> Vec<String> {
        fn walk(value: &mut dyn PartialReflect, path: &str, out: &mut Vec<String>) {
            let mut names = Vec::new();
            match value.reflect_mut() {
                ReflectMut::Struct(s) => names.extend((0..s.field_len()).filter_map(|i| s.name_at(i).map(str::to_string))),
                ReflectMut::Enum(e) => names.extend((0..e.field_len()).map(|i| e.name_at(i).map_or_else(|| i.to_string(), str::to_string))),
                ReflectMut::Tuple(t) => names.extend((0..t.field_len()).map(|i| i.to_string())),
                ReflectMut::Opaque(_) => {
                    if value.try_downcast_ref::<f32>().is_some() || value.try_downcast_ref::<u32>().is_some() {
                        out.push(path.to_string());
                    }
                    return;
                }
                _ => return,
            }
            for name in names {
                if let Some(field) = child(value, &name) {
                    walk(field, &format!("{path}.{name}"), out);
                }
            }
        }
        let mut out = Vec::new();
        walk(value, "", &mut out);
        out
    }

    fn at<'a>(root: &'a mut dyn PartialReflect, path: &str) -> Option<&'a mut dyn PartialReflect> {
        let mut cursor = root;
        for name in path.split('.').skip(1) {
            cursor = child(cursor, name)?;
        }
        Some(cursor)
    }

    /// Set the number at `path` to `value`; a count takes `value` rounded into `0..=u32::MAX` (a `NaN` is 0).
    pub(crate) fn set(root: &mut dyn PartialReflect, path: &str, value: f64) {
        let Some(field) = at(root, path) else { return };
        if let Some(f) = field.try_downcast_mut::<f32>() {
            *f = value as f32;
        } else if let Some(u) = field.try_downcast_mut::<u32>() {
            *u = if value.is_nan() { 0 } else { value.clamp(0.0, u32::MAX as f64) as u32 };
        }
    }

    /// The number at `path`, or `NaN` if there is none.
    pub(crate) fn get(root: &mut dyn PartialReflect, path: &str) -> f64 {
        let Some(field) = at(root, path) else { return f64::NAN };
        if let Some(f) = field.try_downcast_ref::<f32>() {
            return f64::from(*f);
        }
        field.try_downcast_ref::<u32>().map_or(f64::NAN, |u| f64::from(*u))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::TreePreset;

    /// The fields where `a` and `b` differ, for a message.
    fn moved(a: &TreeParams, b: &TreeParams) -> Vec<String> {
        let (mut a, mut b) = (a.clone(), b.clone());
        let paths = fields::numeric(&mut a);
        paths.into_iter().filter(|path| fields::get(&mut a, path) != fields::get(&mut b, path)).collect()
    }

    #[test]
    fn every_preset_and_the_default_are_inside_every_range() {
        assert!(matches!(TreeParams::default().sanitized(), Cow::Borrowed(_)));
        for preset in TreePreset::ALL {
            let params = preset.params();
            let clean = params.sanitized();
            assert!(
                matches!(clean, Cow::Borrowed(_)),
                "{} has fields outside their ranges: {:?}",
                preset.name(),
                moved(&params, &clean)
            );
        }
    }

    #[test]
    fn a_nan_reads_as_the_default_and_the_rest_as_the_nearest_end() {
        let mut params = TreePreset::Oak.params();
        params.trunk.height = f32::NAN;
        params.branches.density = f32::INFINITY;
        params.branches.start = -3.0;
        params.trunk.radial_segments = u32::MAX;
        params.foliage.autumn = Some(Color::srgb(f32::NAN, 0.0, 0.0));
        let clean = params.sanitized();
        assert_eq!(clean.trunk.height, TreeParams::default().trunk.height);
        assert_eq!(clean.branches.density, 1000.0);
        assert_eq!(clean.branches.start, 0.0);
        assert_eq!(clean.trunk.radial_segments, 64);
        assert_eq!(clean.foliage.autumn, None);
        // What was in range is as it was, and the result is in range.
        assert_eq!(clean.trunk.radius, params.trunk.radius);
        assert!(matches!(clean.sanitized(), Cow::Borrowed(_)));
    }

    #[test]
    fn every_numeric_field_is_limited() {
        // A table that forgot a field leaves it at the hostile value.
        let mut base = TreePreset::Apple.params();
        base.crown = CrownStyle::Hull { tiers: 3, sides: 5, lift: 0.1, jitter: 0.1, trunk_only: false };
        base.trunk_style = TrunkStyle::Prism { sides: 5 };
        let paths = fields::numeric(&mut base);
        assert!(paths.len() > 90, "found {} numeric fields", paths.len());
        for path in &paths {
            for hostile in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e30, -1e30] {
                let mut params = base.clone();
                fields::set(&mut params, path, hostile);
                let mut clean = params.sanitized().into_owned();
                // Whatever is left (a `NaN` colour is gone, as `None`) is finite, and a size is no bigger
                // than any range: a colour's channel may be any finite number.
                for left in fields::numeric(&mut clean) {
                    let value = fields::get(&mut clean, &left);
                    let is_colour = ["color", "tint", "autumn", "blossom"].iter().any(|word| left.contains(word));
                    assert!(value.is_finite(), "{path} = {hostile} left {left} = {value}");
                    assert!(is_colour || value.abs() <= 1.0e4, "{path} = {hostile} left {left} = {value}");
                }
            }
        }
    }

    /// A mesh a renderer can take: finite, unit normals, indices inside the vertices, whole triangles.
    fn assert_sound(mesh: &crate::mesh::MeshData, what: &str) {
        assert!(mesh.positions.iter().all(|p| p.is_finite()), "{what}: a position is not finite");
        assert!(mesh.normals.iter().all(|n| n.is_finite()), "{what}: a normal is not finite");
        assert!(mesh.indices.iter().all(|&i| (i as usize) < mesh.positions.len()), "{what}: an index is out of range");
        assert_eq!(mesh.indices.len() % 3, 0, "{what}: not whole triangles");
    }

    fn assert_tree_sound(tree: &crate::TreeMeshes, what: &str) {
        assert_sound(&tree.trunk, what);
        assert_sound(&tree.foliage, what);
        assert_sound(&tree.flowers, what);
        assert!(!tree.skeleton.nodes.is_empty() && tree.skeleton.nodes.len() <= MAX_NODES, "{what}: {} nodes", tree.skeleton.nodes.len());
    }

    /// A birch, the cheapest tree with leaves, that also has every optional part set.
    fn birch_with_everything() -> TreeParams {
        let mut params = TreePreset::Birch.params();
        params.foliage.fruit = Some(FruitParams::default());
        params.foliage.autumn = Some(Color::srgb(0.8, 0.3, 0.1));
        params.foliage.blossom = Some(Color::srgb(1.0, 0.8, 0.9));
        params
    }

    #[test]
    fn a_hostile_value_in_any_field_still_builds_a_sound_tree() {
        use crate::{LodSettings, build_lods, build_tree};
        let base = birch_with_everything();
        let mut probe = base.clone();
        let paths = fields::numeric(&mut probe);
        for path in &paths {
            for hostile in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 1e30, f64::from(u32::MAX)] {
                let mut params = base.clone();
                fields::set(&mut params, path, hostile);
                let what = format!("{path} = {hostile}");
                assert_tree_sound(&build_tree(&params, f32::INFINITY), &what);
                // Young as well (the aged skeleton), with the levels of detail.
                for level in &build_lods(&params, 3.0, &LodSettings::default()) {
                    assert_tree_sound(level, &what);
                }
            }
        }
    }

    #[test]
    fn a_hull_and_a_prism_with_hostile_numbers_build_too() {
        use crate::build_tree;
        let mut base = TreePreset::Spruce.params();
        base.crown = CrownStyle::Hull { tiers: 4, sides: 6, lift: 0.1, jitter: 0.2, trunk_only: false };
        base.trunk_style = TrunkStyle::Prism { sides: 5 };
        let mut probe = base.clone();
        let paths: Vec<String> = fields::numeric(&mut probe).into_iter().filter(|p| p.starts_with(".crown") || p.starts_with(".trunk_style")).collect();
        assert_eq!(paths.len(), 5, "{paths:?}");
        for path in &paths {
            for hostile in [f64::NAN, f64::INFINITY, -1.0, f64::from(u32::MAX)] {
                let mut params = base.clone();
                fields::set(&mut params, path, hostile);
                assert_tree_sound(&build_tree(&params, f32::INFINITY), &format!("{path} = {hostile}"));
            }
        }
    }

    /// `base` with every size (not an angle, a fraction or a colour) at the top of its range.
    fn everything_at_its_largest(base: &TreeParams) -> TreeParams {
        let mut params = base.clone();
        for path in fields::numeric(&mut params.clone()) {
            if ["color", "tint", "autumn", "blossom", "seed", "angle", "twist", "start", "gravity", "droop", "bias", "curve", "bend", "up_attraction", "variation"]
                .iter()
                .any(|word| path.contains(word))
            {
                continue;
            }
            fields::set(&mut params, &path, 1e30);
        }
        params
    }

    #[test]
    fn the_leaves_the_ranges_allow_are_cut_at_the_leaf_budget() {
        use crate::build_tree;
        use crate::foliage::collect_leaf_points;
        let mut base = birch_with_everything();
        base.foliage.placement = crate::FoliagePlacement::AllBranches;
        let params = everything_at_its_largest(&base);
        let clean = params.sanitized();
        assert_eq!((clean.trunk.height, clean.branches.sub_branch_count, clean.foliage.density), (500.0, 32, 512.0));
        let tree = build_tree(&params, f32::INFINITY);
        assert_tree_sound(&tree, "every size at its largest");
        let leaves = collect_leaf_points(&tree.skeleton, &clean.foliage, &clean.growth, &clean.trunk, clean.seed).len();
        assert!(leaves <= MAX_LEAVES && leaves > MAX_LEAVES / 2, "{leaves} leaves for a budget of {MAX_LEAVES}");
    }

    #[test]
    fn the_wood_the_ranges_allow_is_cut_at_the_node_budget() {
        use crate::build_tree;
        let mut base = birch_with_everything();
        base.foliage.enabled = false;
        let params = everything_at_its_largest(&base);
        let tree = build_tree(&params, f32::INFINITY);
        assert_tree_sound(&tree, "every size at its largest, no leaves");
        assert!(tree.skeleton.nodes.len() > MAX_NODES * 9 / 10, "{} nodes", tree.skeleton.nodes.len());
    }

    #[test]
    fn an_age_of_nan_is_an_age_of_zero() {
        use crate::build_tree;
        let params = TreePreset::Oak.params();
        let nan = build_tree(&params, f32::NAN);
        let zero = build_tree(&params, 0.0);
        assert_eq!(nan.trunk.positions, zero.trunk.positions);
        assert_eq!(nan.foliage.positions, zero.foliage.positions);
    }

    #[test]
    fn no_tree_the_crate_draws_comes_near_the_budget() {
        use crate::foliage::leaf_bound;
        for preset in TreePreset::ALL {
            let params = preset.params();
            let skeleton = crate::generate_skeleton(&params);
            let bound: usize = skeleton.nodes.iter().map(|n| leaf_bound(&params.foliage, n.length, n.height_ratio)).sum();
            assert!(skeleton.nodes.len() * 4 <= MAX_NODES, "{}: {} nodes", preset.name(), skeleton.nodes.len());
            assert!(bound * 4 <= MAX_LEAVES, "{}: could bear {bound} leaves", preset.name());
        }
    }

    #[test]
    fn a_tube_never_has_more_sides_than_the_range_says() {
        // `skeleton_mesh` is public and takes parameters from a caller: 4 billion sides is 64.
        let mut params = TreePreset::Oak.params();
        params.trunk.radial_segments = u32::MAX;
        params.trunk.height_segments = u32::MAX;
        params.mesh.smooth_iterations = u32::MAX;
        params.mesh.adaptive_resolution = true;
        params.mesh.min_radial_segments = u32::MAX;
        params.mesh.max_radial_segments = u32::MAX;
        let skeleton = crate::generate_skeleton(&TreePreset::Oak.params());
        assert_sound(&crate::mesh::branch::skeleton_mesh(&skeleton, &params), "skeleton_mesh");
    }
}
