// swift-tools-version: 6.0
// The rr_dr60 C API as a Swift package. Build the binary first: scripts/build-xcframework.sh.
import PackageDescription

let package = Package(
    name: "RRDR60",
    platforms: [.iOS(.v16), .macOS(.v13)],
    products: [
        // The C API (include/rr_dr60.h) as the Swift module RRDR60C.
        .library(name: "RRDR60C", targets: ["RRDR60C"]),
    ],
    targets: [
        .binaryTarget(name: "RRDR60C", path: "RRDR60C.xcframework"),
        .testTarget(name: "RRDR60CTests", dependencies: ["RRDR60C"]),
    ]
)
