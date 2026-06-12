CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(50) NOT NULL UNIQUE,           
    type VARCHAR(20) NOT NULL DEFAULT 'human',      
    password_hash VARCHAR(100) NOT NULL,            
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    CHECK (type IN ('human', 'bot', 'admin'))
);
