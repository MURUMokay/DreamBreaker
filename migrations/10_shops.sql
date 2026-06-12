CREATE TABLE shops (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cell_id UUID REFERENCES game_cells(id) ON DELETE CASCADE,  
    game_id UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE, 
    offset_value INT NOT NULL DEFAULT 0             
);
