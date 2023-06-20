use anyhow::bail;
use fs_err::File;
use simx::{AwayHome, Date, Game, Inning, Player, Sim, Team};

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
    let shorthand = AwayHome {
        away: teams[0].shorthand.clone(),
        home: teams[1].shorthand.clone(),
    };
    for team in teams {
        sim.add_team(team)?;
    }

    let game = Game::new(game_teams);
    sim.start_day(Date::default(), vec![game])?;
    loop {
        sim.tick();
        let game = &sim.games_today()[0];

        print!(
            "{} {} {} {}  ",
            shorthand.away, game.teams.away.runs, shorthand.home, game.teams.home.runs
        );

        match game.inning {
            Inning::Top(n) | Inning::Mid(n) => print!("▲ {}  ", n),
            Inning::Bottom(n) | Inning::End(n) => print!("▼ {}  ", n),
        }

        let occupied = game.bases_occupied();
        let max = occupied.last().copied().unwrap_or_default().max(3);
        for base in (1..=max).rev() {
            if occupied.contains(&base) {
                print!("◆")
            } else {
                print!("◇");
            }
        }
        print!(" ");

        for (num, range) in [
            (game.balls, 0..3.max(game.balls)),
            (game.strikes, 0..2.max(game.strikes)),
            (game.outs, 0..2.max(game.outs)),
        ] {
            print!(" ");
            for i in range {
                if num > i {
                    print!("●");
                } else {
                    print!("○");
                }
            }
        }

        println!("  {}", game.last_update);

        if game.is_finished() {
            break;
        }
    }

    Ok(())
}
