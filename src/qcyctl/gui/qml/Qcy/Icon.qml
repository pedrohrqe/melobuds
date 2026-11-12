import QtQuick
import QtQuick.Window
import Qcy

// Ícones de traço (geometria do Lucide, ISC), em caixa 24×24.
Item {
    id: root
    property string name: "chevron"
    property color color: Theme.text2
    property real stroke: 1.8
    implicitWidth: 18
    implicitHeight: 18

    readonly property var paths: ({
        "chevron": "M6 9l6 6 6-6",
        "bolt": "M13 2L3 14h9l-1 8 10-12h-9l1-8z",
        "check": "M20 6L9 17l-5-5",
        "x": "M18 6L6 18M6 6l12 12",
        "headphones": "M3 14h3a2 2 0 0 1 2 2v3a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-7a9 9 0 0 1 18 0v7a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3",
        "alert": "M12 9v4M12 17h.01M10.3 3.9L1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z",
        "bluetooth": "M7 7l10 10-5 5V2l5 5L7 17",
        "reset": "M3 12a9 9 0 1 0 3-6.7L3 8M3 3v5h5"
    })

    // SVG renderizado como textura: evita artefatos de Shape dentro de áreas com clip.
    Image {
        anchors.fill: parent
        sourceSize: Qt.size(Math.ceil(root.width * Screen.devicePixelRatio), Math.ceil(root.height * Screen.devicePixelRatio))
        smooth: true
        source: {
            const d = root.paths[root.name] || "";
            const c = root.color.toString();
            const fill = root.name === "bolt" ? c : "none";
            return "data:image/svg+xml;utf8," + encodeURIComponent(
                '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="' + fill +
                '" stroke="' + c + '" stroke-width="' + root.stroke +
                '" stroke-linecap="round" stroke-linejoin="round"><path d="' + d + '"/></svg>');
        }
    }
}
