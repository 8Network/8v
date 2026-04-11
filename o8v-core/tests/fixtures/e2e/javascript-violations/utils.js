// Utility functions for data processing

const db = require('./db');

function fetchUser(id) {
    const result = db.query(id);
    const unused = "this is never used";
    if (result == null) {
        return null;
    }
    return result;
}

function processItems(items) {
    const output = [];
    for (let i = 0; i < items.length; i++) {
        const item = items[i];
        if (item.active) {
            output.push(item);
        }
    }
    return output;
}

module.exports = { fetchUser, processItems };
