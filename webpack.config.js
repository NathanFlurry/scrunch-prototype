const BundleAnalyzerPlugin = require("webpack-bundle-analyzer").BundleAnalyzerPlugin;
const JavaScriptObfuscator = require("webpack-obfuscator");
const WebpackOnBuildPlugin = require("on-build-webpack");
const webpack = require("webpack");
const rimraf = require("rimraf");

module.exports = function(env) {
    return new Promise((resolve) => {
        let config = {
            entry: {
                app: "./client/index.ts",
            },
            output: {
                filename: "[name].js",
                path: __dirname + "/public/js"
            },
            resolve: { // Extend module resolutions to include more file types
                extensions: [".ts", /*".tsx",*/ ".js"]
            },
            module: {
                loaders: [
                    {test: /\.tsx?$/, loader: "ts-loader"}
                ]
            },
            stats: { },
            plugins: [
                // Analyzation of the bundle size
                // new BundleAnalyzerPlugin({
                //     analyzerMode: 'static'
                // }),

                // Splits up NPM modules into a separate bundle
                new webpack.optimize.CommonsChunkPlugin({
                    name: 'static',
                    filename: 'static.js',
                    minChunks(module, count) {
                        var context = module.context;
                        return context && context.indexOf('node_modules') >= 0;
                    }
                })
            ]
        };

        // Add production or development-specific functionality
        if (env && env.production) {
            // Add the plugins
            config.plugins.push(new JavaScriptObfuscator({
                compact: true, // Removes linebreaks
                controlFlowFlattening: false,
                deadCodeInjection: false,
                debugProtection: false,
                debugProtectionInterval: false,
                disableConsoleOutput: false, // For debug output
                mangle: true,
                rotateStringArray: false,
                selfDefending: false,
                stringArray: false,
                stringArrayEncoding: false,
                stringArrayThreshold: 0.2,
                unicodeEscapeSequence: true
            }, []));

            // HACK: Exit with code 0 so the process don't fail; this is because
            // `JavaScriptObfuscator` throws errors but still works
            config.plugins.push(new WebpackOnBuildPlugin((stats) => {
                process.exit(0);
            }));
        } else {
            // Add a sourcemap
            config.devtool = "source-map";
        }

        if (env && env.production) {
            // Remove the existing folder and resolve once complete; this way old files like source maps
            // aren't retained
            rimraf(config.output.path, () => {
                console.log("Removed existing build.");
                resolve(config);
            });
        } else {
            resolve(config);
        }
    });
}
