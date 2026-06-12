CREATE TABLE game_rules (
    game_id UUID PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
    starting_balance BIGINT NOT NULL DEFAULT 1000,  
    max_turns INT NOT NULL DEFAULT 50,             
    target_balance BIGINT NOT NULL DEFAULT 2500     
);
