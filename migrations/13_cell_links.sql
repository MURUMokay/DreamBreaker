-- Связи "Расположена на" из ER между Игровым полем и Собственностью/Магазином.
-- Добавляются отдельно, т.к. на момент создания game_cells таблиц
-- properties и shops ещё не существовало (циклическая зависимость).
--
-- Каждая клетка-собственность ссылается на запись в properties,
-- каждая клетка-магазин — на запись в shops.
ALTER TABLE game_cells
    ADD COLUMN property_id UUID REFERENCES properties(id) ON DELETE SET NULL,
    ADD COLUMN shop_id UUID REFERENCES shops(id) ON DELETE SET NULL;
