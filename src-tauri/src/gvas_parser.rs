use crate::graphql::ID;
use crate::models::{pal::Pal, player::Player};
use uuid::Uuid;

pub fn parse_save_file(_path: &str) -> anyhow::Result<Vec<Player>> {
    println!("DEV: Using mock GVAS parser.");

    let mock_pals_player1 = vec![
        Pal {
            instance_id: ID(Uuid::new_v4()),
            character_id: "Cattiva".to_string(),
            nickname: Some("Whiskers".to_string()),
            level: 5,
        },
        Pal {
            instance_id: ID(Uuid::new_v4()),
            character_id: "Lamball".to_string(),
            nickname: None,
            level: 3,
        },
    ];

    let mock_pals_player2 = vec![Pal {
        instance_id: ID(Uuid::new_v4()),
        character_id: "Chikipi".to_string(),
        nickname: Some("Nugget".to_string()),
        level: 2,
    }];

    let players = vec![
        Player {
            uid: ID(Uuid::new_v4()),
            nickname: "Tristan".to_string(),
            level: 15,
            pals: mock_pals_player1,
        },
        Player {
            uid: ID(Uuid::new_v4()),
            nickname: "Omar".to_string(),
            level: 12,
            pals: mock_pals_player2,
        },
    ];

    Ok(players)
}
