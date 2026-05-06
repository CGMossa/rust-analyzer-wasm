const path = require("path");
const HtmlWebPackPlugin = require("html-webpack-plugin");

module.exports = {
    mode: "development",
    entry: { app: "./index.js" },
    output: {
        globalObject: "self",
        filename: "[name].bundle.js",
        chunkFilename: "[id].[contenthash].bundle.js",
        path: path.resolve(__dirname, "dist"),
        publicPath: "",
        clean: true,
    },
    experiments: { asyncWebAssembly: true },
    module: {
        rules: [
            { test: /\.css$/, use: ["style-loader", "css-loader"] },
            { test: /\.ttf$/, type: "asset/resource" },
            // .rs files. fake_*.rs are emitted as standalone assets and
            // fetched at runtime; example-code.rs is small and inlined.
            // Use oneOf so a file matches exactly one rule.
            {
                test: /\.rs$/,
                oneOf: [
                    {
                        test: /fake_(std|core|alloc)\.rs$/,
                        type: "asset/resource",
                        generator: { filename: "[name].[contenthash][ext]" },
                    },
                    { type: "asset/source" },
                ],
            },
        ],
    },
    plugins: [
        new HtmlWebPackPlugin({
            title: "Rust Analyzer Playground",
            chunks: ["app"],
        }),
    ],
};
