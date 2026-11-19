import SwiftUI
import WebKit

@main
struct BoardApp: App {
    @Environment(\.scenePhase) private var phase

    var body: some Scene {
        WindowGroup {
            WebHost()
                .ignoresSafeArea()
                .background(Color.black)
                .preferredColorScheme(.dark)
                .statusBarHidden(false)
                .onOpenURL { Dashboard.shared.openURL($0) }
                .onChange(of: phase) { p in
                    // the dashboard only asks the server for data while it is on screen
                    if p == .active { Dashboard.shared.start() } else { Dashboard.shared.stop() }
                }
        }
    }
}

struct WebHost: UIViewRepresentable {
    func makeUIView(context: Context) -> WKWebView { Dashboard.shared.web }
    func updateUIView(_ uiView: WKWebView, context: Context) {}
}
