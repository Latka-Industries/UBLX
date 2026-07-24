# Documentation: https://docs.brew.sh/Formula-Cookbook
class Ublx < Formula
  desc "TUI that turns a directory into a flat, navigable catalog with previews and metadata"
  homepage "https://ublx.dev/"
  url "https://github.com/Latka-Industries/UBLX/archive/refs/tags/v0.1.14.tar.gz"
  sha256 "e53309d02056b53d8e3c9c2fe5c4bf0e919ffca12110a8be4d536c7d6dd13682"
  license any_of: ["MIT", "Apache-2.0"]

  depends_on "node" => :build
  depends_on "pkgconf" => :build
  depends_on "rust" => :build
  depends_on "rustup" => :build
  depends_on "wasm-bindgen" => :build

  depends_on "ffmpeg"
  depends_on "hdf5"
  depends_on "netcdf"
  depends_on "poppler"
  depends_on "resvg"
  depends_on "tree"

  def install
    hdf5 = Formula["hdf5"].opt_prefix
    netcdf = Formula["netcdf"].opt_prefix
    ENV["HDF5_DIR"] = hdf5
    ENV["HDF5_ROOT"] = hdf5
    ENV["HDF5_INCLUDE_DIR"] = "#{hdf5}/include"
    ENV["HDF5_LIB_DIR"] = "#{hdf5}/lib"
    ENV["NETCDF_DIR"] = netcdf
    ENV.prepend_path "PKG_CONFIG_PATH", "#{hdf5}/lib/pkgconfig"
    ENV.prepend_path "PKG_CONFIG_PATH", "#{netcdf}/lib/pkgconfig"

    # Embedded serve UI (`--features ui`): wasm32 + Tailwind → dist/, then embed.
    # rustup is keg-only; same pattern as homebrew-core `wasm-bindgen` tests.
    ENV.prepend_path "PATH", Formula["rustup"].opt_bin
    system "rustup", "set", "profile", "minimal"
    system "rustup", "default", "stable"
    system "rustup", "target", "add", "wasm32-unknown-unknown"
    ENV.delete "RUSTFLAGS"
    ENV.delete "CARGO_ENCODED_RUSTFLAGS"

    system "./crates/ublx-web/build.sh"
    system "cargo", "install", *std_cargo_args(features: "ui")
  end

  test do
    assert_match "Usage:", shell_output("#{bin}/ublx --help")
  end
end
