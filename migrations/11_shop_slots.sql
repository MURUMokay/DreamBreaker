CREATE TABLE shop_slots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    shop_id UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,    
    power_up_id UUID REFERENCES power_ups(id) ON DELETE SET NULL,    
    slot_index INT NOT NULL,                        
    status VARCHAR(20) NOT NULL DEFAULT 'available', 
    cost BIGINT NOT NULL,
    UNIQUE (shop_id, slot_index),
    CHECK (status IN ('available', 'sold', 'locked'))
);
