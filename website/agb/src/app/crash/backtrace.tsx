"use client";

import { FC } from "react";
import { useClientValue } from "../useClientValue.hook";
import { styled } from "styled-components";

const BacktraceWrapper = styled.section`
  display: flex;
  gap: 10px;
  justify-content: center;
`;

const getBacktrace = () => window.location.hash.slice(1);

export const BacktraceDisplay: FC = () => {
  const backtrace = useClientValue(getBacktrace) ?? "";

  return (
    <BacktraceWrapper>
      <label>Backtrace:</label>
      <input type="text" value={backtrace} />
      <button
        onClick={() => {
          navigator.clipboard.writeText(backtrace);
        }}
      >
        Copy
      </button>
    </BacktraceWrapper>
  );
};
