DROP FUNCTION IF EXISTS init_game_board(UUID, BIGINT);
CREATE OR REPLACE FUNCTION init_game_board(p_game_id UUID, p_seed BIGINT)
RETURNS VOID AS $$
DECLARE
    
    v_names TEXT[] := ARRAY[
        
        'Парк Маримор', 'Центр Маримор', 'Рынок Маримор',
        'Район Маримор', 'Сад Маримор', 'Площадь Маримор', 'Башня Маримор',
        
        'Парк Эльдор', 'Центр Эльдор', 'Рынок Эльдор',
        'Район Эльдор', 'Сад Эльдор', 'Площадь Эльдор', 'Башня Эльдор',
        
        'Парк Кварин', 'Центр Кварин', 'Рынок Кварин',
        'Район Кварин', 'Сад Кварин', 'Площадь Кварин', 'Башня Кварин',
        
        'Парк Зорфис', 'Центр Зорфис', 'Рынок Зорфис',
        'Район Зорфис', 'Сад Зорфис', 'Площадь Зорфис', 'Башня Зорфис',
        
        'Парк Нелвар', 'Центр Нелвар', 'Рынок Нелвар',
        'Район Нелвар', 'Сад Нелвар', 'Площадь Нелвар', 'Башня Нелвар',
        
        'Парк Ауриен', 'Центр Ауриен', 'Рынок Ауриен',
        'Район Ауриен', 'Сад Ауриен', 'Площадь Ауриен', 'Башня Ауриен',
        
        'Парк Тирмас', 'Центр Тирмас', 'Рынок Тирмас',
        'Район Тирмас', 'Сад Тирмас', 'Площадь Тирмас', 'Башня Тирмас',
        
        'Парк Солвэн', 'Центр Солвэн', 'Рынок Солвэн',
        'Район Солвэн', 'Сад Солвэн', 'Площадь Солвэн', 'Башня Солвэн'
    ];

    v_purchase_costs BIGINT[] := ARRAY[
        100, 119, 137, 156, 174, 193, 211,   
        230, 248, 267, 285, 304, 322, 341,   
        359, 378, 396, 415, 433, 452, 470,   
        489, 507, 526, 544, 563, 581, 600    
    ];

    v_side INT;
    v_cell_id UUID;
    v_prop_id UUID;
    v_shop_id UUID;
    v_cell_index INT;
    v_prop_counter INT := 0;  
    v_shop_counter INT := 0;  
    v_color_group INT;
    v_purchase_cost BIGINT;
    v_rent_cost BIGINT;

    v_power_up_ids UUID[];
    v_shuffled_ids UUID[];
    v_shop_ids UUID[];
    v_slot INT;
    v_pu_index INT;
BEGIN
    
    FOR v_side IN 0..3 LOOP
        FOR v_slot IN 0..9 LOOP
            v_cell_index := v_side * 10 + v_slot;

            IF v_slot = 0 THEN
                
                IF v_side = 0 THEN
                    INSERT INTO game_cells (game_id, cell_index, cell_type)
                    VALUES (p_game_id, v_cell_index, 'start')
                    RETURNING id INTO v_cell_id;
                ELSE
                    INSERT INTO game_cells (game_id, cell_index, cell_type)
                    VALUES (p_game_id, v_cell_index, 'shop')
                    RETURNING id INTO v_cell_id;

                    INSERT INTO shops (cell_id, game_id, offset_value)
                    VALUES (v_cell_id, p_game_id, 0)
                    RETURNING id INTO v_shop_id;

                    UPDATE game_cells SET shop_id = v_shop_id WHERE id = v_cell_id;
                    v_shop_counter := v_shop_counter + 1;
                END IF;

            ELSIF v_slot IN (1, 2, 3) THEN
                
                v_color_group := v_side * 2 + 1;  
                v_purchase_cost := v_purchase_costs[v_prop_counter + 1];
                v_rent_cost := CEIL(v_purchase_cost * 0.11);

                INSERT INTO game_cells (game_id, cell_index, cell_type)
                VALUES (p_game_id, v_cell_index, 'property')
                RETURNING id INTO v_cell_id;

                INSERT INTO properties (cell_id, name, purchase_cost, rent_cost)
                VALUES (
                    v_cell_id,
                    v_names[v_prop_counter + 1],
                    v_purchase_cost,
                    v_rent_cost
                )
                RETURNING id INTO v_prop_id;

                UPDATE game_cells SET property_id = v_prop_id WHERE id = v_cell_id;
                v_prop_counter := v_prop_counter + 1;

            ELSIF v_slot = 4 THEN
                
                INSERT INTO game_cells (game_id, cell_index, cell_type)
                VALUES (p_game_id, v_cell_index, 'shop')
                RETURNING id INTO v_cell_id;

                INSERT INTO shops (cell_id, game_id, offset_value)
                VALUES (v_cell_id, p_game_id, 0)
                RETURNING id INTO v_shop_id;

                UPDATE game_cells SET shop_id = v_shop_id WHERE id = v_cell_id;
                v_shop_counter := v_shop_counter + 1;

            ELSIF v_slot = 5 THEN
                
                INSERT INTO game_cells (game_id, cell_index, cell_type, tax_amount)
                VALUES (p_game_id, v_cell_index, 'tax', 0);
                
            ELSIF v_slot IN (6, 7, 8, 9) THEN
                
                v_color_group := v_side * 2 + 2;  
                v_purchase_cost := v_purchase_costs[v_prop_counter + 1];
                v_rent_cost := CEIL(v_purchase_cost * 0.11);

                INSERT INTO game_cells (game_id, cell_index, cell_type)
                VALUES (p_game_id, v_cell_index, 'property')
                RETURNING id INTO v_cell_id;

                INSERT INTO properties (cell_id, name, purchase_cost, rent_cost)
                VALUES (
                    v_cell_id,
                    v_names[v_prop_counter + 1],
                    v_purchase_cost,
                    v_rent_cost
                )
                RETURNING id INTO v_prop_id;

                UPDATE game_cells SET property_id = v_prop_id WHERE id = v_cell_id;
                v_prop_counter := v_prop_counter + 1;
            END IF;
        END LOOP;
    END LOOP;

    SELECT ARRAY(SELECT id FROM power_ups ORDER BY cost) INTO v_power_up_ids;

    SELECT ARRAY(
        SELECT id FROM power_ups
        ORDER BY md5((p_seed + ROW_NUMBER() OVER (ORDER BY cost))::TEXT)
    ) INTO v_shuffled_ids;

    SELECT ARRAY(SELECT id FROM shops WHERE game_id = p_game_id ORDER BY id)
    INTO v_shop_ids;

    FOR v_shop_counter IN 1..array_length(v_shop_ids, 1) LOOP
        FOR v_slot IN 0..3 LOOP
            v_pu_index := ((v_shop_counter - 1) * 4 + v_slot)
                          % array_length(v_shuffled_ids, 1) + 1;

            INSERT INTO shop_slots (shop_id, power_up_id, slot_index, status, cost)
            VALUES (
                v_shop_ids[v_shop_counter],
                v_shuffled_ids[v_pu_index],
                v_slot,
                'available',
                (SELECT cost FROM power_ups WHERE id = v_shuffled_ids[v_pu_index])
            );
        END LOOP;
    END LOOP;
END;
$$ LANGUAGE plpgsql;

DROP FUNCTION IF EXISTS get_board_cells(UUID);
CREATE OR REPLACE FUNCTION get_board_cells(p_game_id UUID)
RETURNS TABLE (
    cell_index  INT,
    cell_type   TEXT,
    tax_amount  BIGINT,
    
    prop_name        TEXT,
    purchase_cost    BIGINT,
    rent_cost        BIGINT,
    owner_user_id    UUID,
    
    shop_id          UUID,
    refresh_cost     BIGINT
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        gc.cell_index,
        gc.cell_type::TEXT,
        gc.tax_amount,
        p.name::TEXT,
        p.purchase_cost,
        p.rent_cost,
        p.owner_user_id,
        s.id,
        NULL::BIGINT  
    FROM game_cells gc
    LEFT JOIN properties p ON p.id = gc.property_id
    LEFT JOIN shops s ON s.id = gc.shop_id
    WHERE gc.game_id = p_game_id
    ORDER BY gc.cell_index;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

DROP FUNCTION IF EXISTS get_participant_state(UUID, UUID);
CREATE OR REPLACE FUNCTION get_participant_state(p_game_id UUID, p_user_id UUID)
RETURNS TABLE (
    "position"   INT,
    balance      BIGINT,
    moves_made   INT,
    total_spent  BIGINT,
    total_earned BIGINT
) AS $$
BEGIN
    RETURN QUERY
    SELECT gp."position", gp.balance, gp.moves_made, gp.total_spent, gp.total_earned
    FROM game_participants gp
    WHERE gp.game_id = p_game_id AND gp.user_id = p_user_id;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

DROP FUNCTION IF EXISTS calc_rent(UUID, UUID);
CREATE OR REPLACE FUNCTION calc_rent(p_game_id UUID, p_property_id UUID)
RETURNS BIGINT AS $$
DECLARE
    v_base_cost     BIGINT;
    v_owner_user_id UUID;
    v_flat_base     BIGINT := 0;
    v_pct_bonus     NUMERIC := 0;
    v_flat_final    BIGINT := 0;
    v_result        BIGINT;
BEGIN
    SELECT p.rent_cost, p.owner_user_id
    INTO v_base_cost, v_owner_user_id
    FROM properties p WHERE p.id = p_property_id;

    IF v_owner_user_id IS NULL THEN
        RETURN 0;
    END IF;

    SELECT
        COALESCE(SUM(CASE WHEN (elem->'effect'->>'type') = 'flat_base'
                     THEN (elem->'effect'->>'value')::BIGINT ELSE 0 END), 0),
        COALESCE(SUM(CASE WHEN (elem->'effect'->>'type') = 'percent_bonus'
                     THEN (elem->'effect'->>'value')::NUMERIC ELSE 0 END), 0),
        COALESCE(SUM(CASE WHEN (elem->'effect'->>'type') = 'flat_final'
                     THEN (elem->'effect'->>'value')::BIGINT ELSE 0 END), 0)
    INTO v_flat_base, v_pct_bonus, v_flat_final
    FROM properties p
    CROSS JOIN jsonb_array_elements(p.upgrades) AS elem
    WHERE p.id = p_property_id;

    v_result := v_base_cost
                + v_flat_base
                + CEIL(v_base_cost * v_pct_bonus / 100.0)
                + v_flat_final;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

DROP FUNCTION IF EXISTS create_game_full(BIGINT, BIGINT, INT, BIGINT, UUID);
CREATE OR REPLACE FUNCTION create_game_full(
    p_seed             BIGINT,
    p_starting_balance BIGINT,
    p_max_turns        INT,
    p_target_balance   BIGINT,
    p_user_id          UUID
) RETURNS UUID AS $$
DECLARE
    v_game_id UUID;
BEGIN
    
    INSERT INTO games (seed) VALUES (p_seed) RETURNING id INTO v_game_id;

    INSERT INTO game_rules (game_id, starting_balance, max_turns, target_balance)
    VALUES (v_game_id, p_starting_balance, p_max_turns, p_target_balance);

    INSERT INTO game_participants (game_id, user_id, turn_order, balance)
    VALUES (v_game_id, p_user_id, 1, p_starting_balance);

    INSERT INTO game_participants (game_id, user_id, turn_order, balance)
    SELECT v_game_id, u.id, ROW_NUMBER() OVER () + 1, p_starting_balance
    FROM users u
    WHERE u.type = 'bot'
    LIMIT 4;

    PERFORM init_game_board(v_game_id, p_seed);

    UPDATE games SET status = 'active' WHERE id = v_game_id;

    RETURN v_game_id;
END;
$$ LANGUAGE plpgsql;
