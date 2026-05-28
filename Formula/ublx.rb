# Documentation: https://docs.brew.sh/Formula-Cookbook
class Ublx < Formula
  desc "TUI that turns a directory into a flat, navigable catalog with previews and metadata"
  homepage "https://github.com/Latka-Industries/UBLX"
  url "https://github.com/Latka-Industries/UBLX/archive/refs/tags/v0.1.7.tar.gz"
  sha256 "d1a6b50e503ad1e2b23568bb4f82cca886aa7a9a87a5d573a96ce774eb3c16b2"
  license any_of: ["MIT", "Apache-2.0"]

  depends_on "pkgconf" => :build
  depends_on "rust" => :build

  depends_on "ffmpeg"
  depends_on "hdf5"
  depends_on "netcdf"
  depends_on "poppler"
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
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "Usage:", shell_output("#{bin}/ublx --help")
  end
end
