if FORMAT:match("latex") then
  function HorizontalRule()
    return {
      pandoc.RawBlock("latex", "\\noindent\\rule{\\linewidth}{0.5pt}")
    }
  end
end