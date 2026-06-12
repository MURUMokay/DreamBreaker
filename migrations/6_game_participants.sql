CREATE TABLE game_participants (
    game_id UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    position INT NOT NULL DEFAULT 0,                
    balance BIGINT NOT NULL DEFAULT 1000,           
    moves_made INT NOT NULL DEFAULT 0,              
    total_spent BIGINT NOT NULL DEFAULT 0,          
    total_earned BIGINT NOT NULL DEFAULT 0,         
    turn_order INT NOT NULL,
    PRIMARY KEY (game_id, user_id)
);
