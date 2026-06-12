CREATE TABLE power_ups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,                     
    description TEXT,                               
    cost BIGINT NOT NULL,                           
    effect JSONB NOT NULL                           
);
