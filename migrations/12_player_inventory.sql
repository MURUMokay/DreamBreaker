CREATE TABLE player_inventory (
    game_id UUID NOT NULL,
    user_id UUID NOT NULL,
    power_up_id UUID NOT NULL REFERENCES power_ups(id) ON DELETE CASCADE,
    quantity INT NOT NULL DEFAULT 1,
    PRIMARY KEY (game_id, user_id, power_up_id),
    FOREIGN KEY (game_id, user_id)
        REFERENCES game_participants(game_id, user_id) ON DELETE CASCADE
);
