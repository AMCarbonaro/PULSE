// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "PulseSDK",
    platforms: [
        .iOS(.v15),
        .watchOS(.v8)
    ],
    products: [
        .library(
            name: "PulseSDK",
            targets: ["PulseSDK"]
        ),
    ],
    dependencies: [
        // secp256k1 for signing (matches node's k256 crate)
        .package(url: "https://github.com/GigaBitcoin/secp256k1.swift.git", exact: "0.17.0"),
    ],
    targets: [
        .target(
            name: "PulseSDK",
            dependencies: [
                .product(name: "secp256k1", package: "secp256k1.swift"),
            ],
            path: "Sources"
        ),
        .testTarget(
            name: "PulseSDKTests",
            dependencies: ["PulseSDK"],
            path: "Tests"
        ),
    ]
)
