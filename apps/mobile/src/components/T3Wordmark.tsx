import type { ColorValue } from "react-native";
import Svg, { Path } from "react-native-svg";
import { withUniwind } from "uniwind";

const ThemedPath = withUniwind(Path);

/**
 * The Backplane mark used by the web sidebar and native app chrome.
 */
export function T3Wordmark(props: {
  readonly height: number;
  readonly color?: ColorValue;
  readonly colorClassName?: string;
}) {
  return (
    <Svg
      accessibilityLabel="Backplane"
      height={props.height}
      width={props.height}
      viewBox="0 0 256 256"
    >
      <ThemedPath
        d="m122 40-83 67v80l83-67Zm39 12-83 67v80l83-67Zm39 12-83 67v80l83-67Z"
        color={props.color}
        colorClassName={props.colorClassName}
        fill="currentColor"
      />
    </Svg>
  );
}
