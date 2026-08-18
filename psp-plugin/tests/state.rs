use psp_plugin::state::LuaState;

#[test]
fn lua_state_opens_and_reports_an_empty_stack() {
    let state = LuaState::new().expect("a state must open");
    assert_eq!(state.stack_top(), 0);
}

#[test]
fn repeated_open_and_close_cycles_stay_healthy() {
    for _ in 0..64 {
        let state = LuaState::new().expect("a state must open");
        assert_eq!(state.stack_top(), 0);
    }
}
