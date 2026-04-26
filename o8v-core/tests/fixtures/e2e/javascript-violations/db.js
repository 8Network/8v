// Database stub

const poolSize = 10;

function query(id) {
    console.log("querying for", id);
    if (poolSize == 0) {
        return null;
    }
    return { id: id, name: "test" };
}

module.exports = { query };
