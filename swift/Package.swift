// swift-tools-version: 6.0
// The rr_dr60 C API as a Swift package. Build the binary first: scripts/build-xcframework.sh.
import PackageDescription

let package = Package(
    name: "RRDR60",
    platforms: [.iOS(.v16), .macOS(.v13)],
    products: [
        // The C API (include/rr_dr60.h) as the Swift module RRDR60C.
        .library(name: "RRDR60C", targets: ["RRDR60C"]),
        // The Swift helpers over it (spec 020: the processing record's re-render recipe).
        .library(name: "RRDR60", targets: ["RRDR60"]),
    ],
    targets: [
        .binaryTarget(name: "RRDR60C", path: "RRDR60C.xcframework"),
        .target(name: "RRDR60", dependencies: ["RRDR60C"]),
        .testTarget(name: "RRDR60CTests", dependencies: ["RRDR60C"]),
        .testTarget(name: "RRDR60Tests", dependencies: ["RRDR60", "RRDR60C"]),
    ]
)
