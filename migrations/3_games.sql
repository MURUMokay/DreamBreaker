CREATE TABLE games (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    status VARCHAR(20) NOT NULL DEFAULT 'pending',  
    seed BIGINT NOT NULL,                           
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),  
    last_saved_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), 
    CHECK (status IN ('pending', 'active', 'paused', 'finished', 'surrender'))
);
