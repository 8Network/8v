// Main application entry point

const utils = require('./utils');

const config = {
    port: 3000
};

var unusedVar = "never used";

function main() {
    const items = [
        { id: 1, active: true },
        { id: 2, active: false }
    ];

    console.log("Starting app on port", config.port);

    const result = utils.processItems(items);
    if (result.length == 0) {
        return null;
    }

    return result;
}

main();
