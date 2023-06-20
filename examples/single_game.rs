use anyhow::bail;
use fs_err::File;
use simx::{AwayHome, Date, Game, Player, Sim, Team};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let (Some(players_file), Some(teams_file)) = (args.next(), args.next()) else {
        bail!("missing arguments\nusage: cargo run --example single_game -- PLAYERS.JSON TEAMS.JSON");
    };

    let mut sim = Sim::new();

    let players: Vec<Player> = serde_json::from_reader(File::open(players_file)?)?;
    for player in players {
        sim.add_player(player)?;
    }
    let teams: Vec<Team> = serde_json::from_reader(File::open(teams_file)?)?;
    let game_teams = AwayHome {
        away: teams[0].id,
        home: teams[1].id,
    };
    for team in teams {
        sim.add_team(team)?;
    }

    let game = Game::new(game_teams);
    sim.start_day(Date::default(), vec![game])?;
    loop {
        sim.tick();
        let game = &sim.games_today()[0];
        println!("{}", game.last_update);
        if game.is_finished() {
            break;
        }
    }

    Ok(())
}
