.pragma library

// Spreadsheet intake stays a preview until the workspace explicitly appends these rows.
function utf8Length(text) {
    let bytes = 0
    for (let i = 0; i < text.length; ++i) {
        const code = text.charCodeAt(i)
        if (code >= 0xd800 && code <= 0xdbff) {
            const low = text.charCodeAt(++i)
            if (!(low >= 0xdc00 && low <= 0xdfff)) return -1
            bytes += 4
        } else if (code >= 0xdc00 && code <= 0xdfff) return -1
        else bytes += code < 0x80 ? 1 : code < 0x800 ? 2 : 3
    }
    return bytes
}
function failure(error, line) {
    return {rows: [], error: error, line: line || 0, duplicateCount: 0, headerSkipped: false}
}
function effectiveCards(composition) {
    const cards = composition.cards || []
    const card = cards.length === 1 ? cards[0] : null
    const untouched = card && card.quantity === 1 && !card.artwork && !card.text_material && !card.layout
        && ["title", "card_type", "cost", "rules", "stats", "flavor", "art_prompt"]
            .every(key => card[key] === undefined || card[key] === "")
    return untouched ? [] : cards
}
function parse(text, composition, firstRowHeader) {
    const bytes = utf8Length(text)
    if (bytes < 0) return failure("invalid_text", 1)
    if (bytes > 60 * 1024) return failure("too_large")
    text = text.replace(/\r\n?/g, "\n")
    const raw = []
    let cells = [], field = "", quoted = false, closedQuote = false
    let line = 1, startLine = 1, error = ""
    function pushRow() {
        cells.push(field); field = ""; closedQuote = false
        if (cells.some(value => value.trim().length)) raw.push({cells: cells, line: startLine})
        cells = []
        if (raw.length > 65) error = "too_many_cards"
    }
    for (let i = 0; i < text.length && !error.length; ++i) {
        const ch = text[i]
        if (quoted) {
            if (ch === '"') {
                if (text[i + 1] === '"') { field += '"'; ++i }
                else { quoted = false; closedQuote = true }
            } else { field += ch; if (ch === "\n") ++line }
        } else if (ch === "\t") {
            cells.push(field); field = ""; closedQuote = false
        } else if (ch === "\n") {
            pushRow(); ++line; startLine = line
        } else if (closedQuote) {
            if (ch !== " ") error = "quote"
        } else if (ch === '"' && field.length === 0) quoted = true
        else field += ch
    }
    if (quoted || error === "quote") return failure("quote", line)
    if (error.length) return failure(error, line)
    pushRow()
    if (error.length) return failure(error, line)
    let headerSkipped = false
    let columns = ["title", "rules", "art_prompt"]
    if (raw.length) {
        const header = raw[0].cells.map(value => value.trim().toLowerCase())
        const aliases = {
            title: ["title", "name", "标题", "卡名", "名称"],
            rules: ["text", "body", "rules", "description", "正文", "文字", "规则", "描述", "说明"],
            art_prompt: ["artwork description", "artwork", "art_prompt", "插画描述", "插图描述", "插图"]
        }
        const mapped = header.map(value => Object.keys(aliases).find(key => aliases[key].indexOf(value) >= 0))
        const recognized = header.length <= 3 && mapped.every(key => key) && new Set(mapped).size === mapped.length
        if (firstRowHeader && !recognized) return failure("heading", raw[0].line)
        if (recognized && (firstRowHeader || (raw.length > 1 && header.length >= 2))) {
            columns = mapped; raw.shift(); headerSkipped = true
        }
    }
    if (!raw.length) return failure("empty")
    const existing = effectiveCards(composition)
    if (existing.length + raw.length > 64) return failure("too_many_cards")
    const seen = Object.create(null), rows = []
    let duplicateCount = 0
    for (const card of existing) {
        const key = ((card.title || "").trim() || (card.rules || "").trim()).toLowerCase()
        if (key.length) seen[key] = true
    }
    for (const item of raw) {
        if (item.cells.length > 3) return failure("columns", item.line)
        const row = {title: "", rules: "", art_prompt: ""}
        for (let index = 0; index < item.cells.length; ++index) {
            if (!columns[index]) return failure("columns", item.line)
            row[columns[index]] = item.cells[index]
        }
        row.title = row.title.trim()
        if (!row.title.length && !row.rules.trim().length) return failure("content", item.line)
        if (/[\u0000-\u001f\u007f-\u009f]/.test(row.title)
            || /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f-\u009f]/.test(row.rules + row.art_prompt))
            return failure("invalid_text", item.line)
        if (utf8Length(row.title) > 256 || utf8Length(row.rules) > 8192 || utf8Length(row.art_prompt) > 4096)
            return failure("field_length", item.line)
        const key = (row.title || row.rules.trim()).toLowerCase()
        if (Object.prototype.hasOwnProperty.call(seen, key)) ++duplicateCount
        seen[key] = true; rows.push(row)
    }
    const estimate = JSON.parse(JSON.stringify(composition))
    estimate.cards = existing.concat(rows.map(row => {
        const card = {id: "x".repeat(64), title: row.title, rules: row.rules, art_prompt: row.art_prompt,
            card_type: "", cost: "", stats: "", flavor: "", quantity: 1, artwork: null, text_material: null}
        if (composition.layout) card.layout = {mode: "text"}
        return card
    }))
    if (utf8Length(JSON.stringify(estimate)) > 60 * 1024) return failure("composition_size")
    return {rows: rows, error: "", line: 0, duplicateCount: duplicateCount, headerSkipped: headerSkipped,
        replacesPlaceholder: existing.length < (composition.cards || []).length}
}
