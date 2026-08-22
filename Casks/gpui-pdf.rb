cask "gpui-pdf" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.1.0"
  sha256 arm:   "0000000000000000000000000000000000000000000000000000000000000000",
         intel: "0000000000000000000000000000000000000000000000000000000000000000"

  url "https://github.com/list-jonas/gpui-pdf/releases/download/v#{version}/gpui-pdf-#{version}-#{arch}-apple-darwin.tar.gz",
      verified: "github.com/list-jonas/gpui-pdf/"
  name "GPUI PDF"
  desc "Local-first PDF reader and editor drawn with GPUI"
  homepage "https://github.com/list-jonas/gpui-pdf"

  depends_on macos: ">= :big_sur"

  app "GPUI PDF.app"

  caveats <<~EOS
    The app is ad-hoc signed, not Developer ID signed or notarized.
    Install it with --no-quarantine, or Gatekeeper will refuse to open it:

      brew install --cask --no-quarantine list-jonas/gpui-pdf/gpui-pdf
  EOS

  zap trash: [
    "~/Library/Preferences/com.gpui.pdf.plist",
    "~/Library/Saved Application State/com.gpui.pdf.savedState",
  ]
end
