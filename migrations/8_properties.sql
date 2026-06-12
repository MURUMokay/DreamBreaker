CREATE TABLE properties (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cell_id UUID REFERENCES game_cells(id) ON DELETE CASCADE,  
    
    owner_game_id UUID,
    owner_user_id UUID,
    name VARCHAR(100) NOT NULL,                     
    purchase_cost BIGINT NOT NULL,                  
    rent_cost BIGINT NOT NULL,                      
    upgrades JSONB DEFAULT '[]'::JSONB,             
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (owner_game_id, owner_user_id)
        REFERENCES game_participants(game_id, user_id) ON DELETE SET NULL
);
