CREATE INDEX IF NOT EXISTS idx_participants_game ON game_participants(game_id);
CREATE INDEX IF NOT EXISTS idx_cells_game ON game_cells(game_id);
CREATE INDEX IF NOT EXISTS idx_shops_game ON shops(game_id);
CREATE INDEX IF NOT EXISTS idx_shop_slots_shop ON shop_slots(shop_id);
CREATE INDEX IF NOT EXISTS idx_properties_owner ON properties(owner_game_id, owner_user_id);

CREATE INDEX IF NOT EXISTS idx_games_status ON games(status);
CREATE INDEX IF NOT EXISTS idx_participants_user_game ON game_participants(user_id, game_id);
CREATE INDEX IF NOT EXISTS idx_participants_status ON game_participants(user_id);

CREATE OR REPLACE FUNCTION update_game_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.last_saved_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_auto_save_game
BEFORE UPDATE ON games
FOR EACH ROW
EXECUTE FUNCTION update_game_timestamp();
