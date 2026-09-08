import { useId, type SVGProps } from "react";

export function T3Wordmark(props: SVGProps<SVGSVGElement>) {
  const maskId = `${useId().replaceAll(":", "")}-backplane-mark`;

  return (
    <svg {...props} viewBox="0 0 256 256" xmlns="http://www.w3.org/2000/svg">
      <defs>
        <mask id={maskId}>
          <g strokeLinejoin="miter" transform="translate(256 0) scale(-1 1)">
            <path d="m103 22 128 40v112l-128-40Z" fill="black" stroke="white" strokeWidth="7" />
            <g fill="white" stroke="black" strokeWidth="7">
              <path d="m122 40-83 67v80l83-67Z" />
              <path d="m161 52-83 67v80l83-67Z" />
              <path d="m200 64-83 67v80l83-67Z" />
            </g>
            <g stroke="black" strokeWidth="4">
              <path d="M49 109v66M88 121v66M127 133v66" />
            </g>
          </g>
        </mask>
      </defs>
      <path d="M0 0h256v256H0z" fill="currentColor" mask={`url(#${maskId})`} />
    </svg>
  );
}
