use crate::{models::player::Player, state::AppState};
use async_graphql::{
    Context, EmptyMutation, EmptySubscription, Object, Result, Scalar, ScalarType, Schema, Value,
};
use uuid::Uuid;

// This creates a new type `ID(Uuid)` that we can use in our models.
// It tells GraphQL how to handle our custom ID type by treating it as a string.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct ID(pub Uuid);

#[Scalar]
impl ScalarType for ID {
    fn parse(value: Value) -> async_graphql::InputValueResult<Self> {
        if let Value::String(s) = value {
            Ok(ID(Uuid::parse_str(&s)?))
        } else {
            Err(async_graphql::InputValueError::expected_type(value))
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.0.to_string())
    }
}

pub struct Query;

#[Object]
impl Query {
    /// Returns all players from the loaded save file.
    async fn players(&self, ctx: &Context<'_>) -> Result<Vec<Player>> {
        let state = ctx.data::<AppState>()?.0.lock().unwrap();
        match &state.players {
            Some(players) => Ok(players.clone()),
            None => Ok(vec![]),
        }
    }

    // You can add more queries here as your application grows.
    // For example:
    //
    // async fn player(&self, ctx: &Context<'_>, uid: ID) -> Result<Option<Player>> {
    //     let state = ctx.data::<AppState>()?.0.lock().unwrap();
    //     Ok(state
    //         .players
    //         .as_ref()
    //         .and_then(|players| players.iter().find(|p| p.uid == uid).cloned()))
    // }
}

pub type AppSchema = Schema<Query, EmptyMutation, EmptySubscription>;

pub fn build_schema(state: AppState) -> AppSchema {
    Schema::build(Query, EmptyMutation, EmptySubscription)
        .data(state)
        .finish()
}
