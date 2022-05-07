#[macro_use] extern crate rocket;

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::Parser;
use expect_exit::ExpectedWithError;
use flate2::read::GzDecoder;
use rocket::{Build, Rocket, State};
use rocket::response::status::BadRequest;
use rocket::serde::json::Json;
use rocket_prometheus::PrometheusMetrics;
use serde::{Deserialize, Serialize};

use model::{Coordinate, NavGrid};
use model::definitions::{EdgeDefinition, GameState, RequirementDefinition};
use pathfinder::{BucketRingBuffer, DijkstraCacheState};

#[derive(Parser)]
struct Options {
    /// Path to NavGrid file
    #[clap(short, long)]
    navgrid: PathBuf,
}

#[derive(Deserialize)]
struct Request {
    start: Coordinate,
    end: Coordinate,
    #[serde(default)]
    game_state: GameState,
}

#[derive(Clone, Default, Serialize)]
struct DataSelection {
    varps: HashSet<u32>,
    varbits: HashSet<u32>,
    items: HashSet<String>,
    skills: HashSet<String>,
}

#[post("/", data = "<request>")]
fn handle_path_request(request: Json<Request>, nav_grid: &State<NavGrid>) -> Result<Json<Option<Vec<EdgeDefinition>>>, BadRequest<&str>> {
    if !request.start.validate() || !request.end.validate() {
        println!("[Path] {} -> {} invalid coordinates", request.start, request.end);
        Err(BadRequest(Some("Coordinate out of bounds")))
    } else {
        let begin = Instant::now();
        let (visited, mem_usage, path) = pathfinder::dijkstra(&nav_grid, &request.start, &request.end, &request.game_state);
        let duration = Instant::now() - begin;
        println!("[Path] {} -> {} in {:.2}ms, {}Kb, {} visited", request.start, request.end, duration.as_secs_f64() * 1000f64, mem_usage / 1024, visited);
        Ok(Json(path))
    }
}

#[post("/", data = "<request>")]
fn handle_bench_request(request: Json<Request>, nav_grid: &State<NavGrid>) -> Result<Json<f64>, BadRequest<&str>> {
    if !request.start.validate() || !request.end.validate() {
        println!("[Path] {} -> {} invalid coordinates", request.start, request.end);
        Err(BadRequest(Some("Coordinate out of bounds")))
    } else {
        let max_cost = nav_grid.iter_edges().map(|edge| edge.cost).max().unwrap();
        let mut queue = BucketRingBuffer::new(max_cost); //TODO borrow from pool instead to prevent allocations?
        let mut cache = vec![DijkstraCacheState { cost: u32::MAX, prev: u32::MAX, edge: None }; 6400*4000];
        let begin = Instant::now();
        for _ in 0..100000 {
            unsafe { pathfinder::dijkstra2(&nav_grid, &request.start, &request.end, &request.game_state, &mut queue, &mut cache); }
            queue.reset();
            for x in cache.iter_mut() {
                *x = DijkstraCacheState { cost: u32::MAX, prev: u32::MAX, edge: None };
            }
        }
        let duration = Instant::now() - begin;
        Ok(Json(duration.as_secs_f64()))
    }
}

#[get("/")]
fn handle_select_request(tracked_varps: &State<DataSelection>) -> Json<DataSelection> {
    Json(tracked_varps.inner().clone())
}

#[launch]
fn rocket() -> Rocket<Build> {
    let options = Options::parse();
    let nav_grid = load_nav_grid(&options.navgrid).or_exit_e_("Error loading NavGrid");
    let mut data_selection = DataSelection::default();
    nav_grid.iter_edges().flat_map(|e| &e.requirements).for_each(|r| {
        match r {
            RequirementDefinition::Varp { index, .. } => data_selection.varps.insert(*index),
            RequirementDefinition::Varbit { index, .. } => data_selection.varbits.insert(*index),
            RequirementDefinition::Item { item, .. } => data_selection.items.insert(item.to_string()),
            RequirementDefinition::Skill { skill, .. } => data_selection.skills.insert(skill.clone()),
            _ => false
        };
    });
    let prometheus = PrometheusMetrics::new();
    rocket::build()
        .attach(prometheus.clone())
        .mount("/metrics", prometheus)
        .mount("/path", routes![handle_path_request])
        .mount("/select", routes![handle_select_request])
        .manage(nav_grid)
        .manage(data_selection)
}

fn load_nav_grid(path: impl AsRef<Path>) -> Result<NavGrid, ciborium::de::Error<std::io::Error>> {
    let file = File::open(path)?;
    let decoder = GzDecoder::new(file);
    let mut reader = BufReader::new(decoder);
    let mut nav_grid = NavGrid::new();
    for vertex in &mut nav_grid.vertices {
        let mut buf = [0; 2];
        reader.read_exact(&mut buf)?;
        vertex.flags = buf[0];
        vertex.extra_edges_and_group = buf[1];
    }
    nav_grid.edges = ciborium::de::from_reader(&mut reader)?;
    nav_grid.teleports = ciborium::de::from_reader(&mut reader)?;
    Ok(nav_grid)
}
