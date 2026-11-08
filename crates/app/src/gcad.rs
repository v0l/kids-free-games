use crate::games::runner::world::{Models, PROPS};
use bevy::platform::time::Instant;
use bevy::prelude::*;
use gcad_bevy::prelude::*;
use gcad_bevy::GcadStats;

pub fn settings(scale: Option<f32>) -> GcadSettings {
    GcadSettings {
        height: scale.is_none().then_some(1.0),
        scale: scale.map(f64::from),
        ..GcadSettings::fit_height(1.0)
    }
}

pub fn bake_settings(asset: &str) -> Option<GcadSettings> {
    if asset == "models/hero.gcad" {
        return Some(settings(None));
    }
    let name = asset.strip_prefix("models/prop_")?.strip_suffix(".gcad")?;
    let (_, scale) = PROPS.iter().find(|(p, _)| *p == name)?;
    Some(settings(*scale))
}

pub struct ModelsPlugin;

impl Plugin for ModelsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(GcadPlugin::default())
            .add_systems(Startup, warm_up)
            .add_systems(Update, report_ready.run_if(resource_exists::<Warming>));
    }
}

#[derive(Resource)]
struct Warming {
    started: Instant,
    pending: Vec<&'static str>,
}

fn warm_up(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    let started = Instant::now();
    let models = Models::load(&assets, &mut graphs);
    commands.insert_resource(Warming {
        started,
        pending: models.names().collect(),
    });
    commands.insert_resource(models);
}

fn report_ready(
    mut commands: Commands,
    mut warming: ResMut<Warming>,
    models: Res<Models>,
    assets: Res<AssetServer>,
    stats: Res<GcadStats>,
) {
    let started = warming.started;
    warming.pending.retain(|name| {
        let Some(handle) = models.scene(name) else {
            return false;
        };
        let ready = assets.is_loaded_with_dependencies(handle);
        let failed = matches!(assets.load_state(handle), bevy::asset::LoadState::Failed(_));
        if ready {
            info!(
                "model {name} ready {:.0} ms after start",
                started.elapsed().as_secs_f64() * 1000.0
            );
        }
        !ready && !failed
    });
    if warming.pending.is_empty() {
        info!(
            "every model ready {:.0} ms after start, {} builds, {} baked",
            started.elapsed().as_secs_f64() * 1000.0,
            stats.builds(),
            stats.hits_baked()
        );
        commands.remove_resource::<Warming>();
    }
}
