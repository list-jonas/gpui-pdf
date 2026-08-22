cask "gpui-pdf" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.1.0"
  sha256 arm:   "88ad873a8795f88be1fba2b81090b85492a028d48519235241caae9aa8da7484",
         intel: "df40ced63ffc7660ae5d570ff3b3442f8aa2aab1c05e62336e1cd109a55f3a72"

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
