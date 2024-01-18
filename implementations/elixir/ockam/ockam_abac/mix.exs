defmodule Ockam.ABAC.MixProject do
  use Mix.Project

  @version "0.10.1"

  @elixir_requirement "~> 1.10"

  @ockam_github_repo "https://github.com/build-trust/ockam"
  @ockam_github_repo_path "implementations/elixir/ockam/ockam_abac"

  def project do
    [
      app: :ockam_abac,
      version: @version,
      elixir: @elixir_requirement,
      consolidate_protocols: Mix.env() != :test,
      elixirc_options: [warnings_as_errors: true],
      deps: deps(),
      aliases: aliases(),

      # lint
      dialyzer: [flags: [:error_handling]],

      # test
      test_coverage: [output: "_build/cover"],
      preferred_cli_env: ["test.cover": :test],
      elixirc_paths: elixirc_paths(Mix.env()),

      # hex
      description: "Ockam ABAC",
      package: package(),

      # docs
      name: "Ockam ABAC",
      docs: docs()
    ]
  end

  # mix help compile.app for more
  def application do
    [
      mod: {Ockam.ABAC, []},
      extra_applications: [:logger, :ockam]
    ]
  end

  defp deps do
    [
      {:credo, "~> 1.6.0", only: [:dev, :test], runtime: false},
      {:dialyxir, "~> 1.1", only: [:dev], runtime: false},
      {:ex_doc, "~> 0.25", only: :dev, runtime: false},
      {:ockam, path: "../ockam"},
      {:neotoma, git: "https://github.com/seancribbs/neotoma.git", runtime: false}
    ]
  end

  # used by hex
  defp package do
    [
      links: %{"GitHub" => @ockam_github_repo},
      licenses: ["Apache-2.0"]
    ]
  end

  defp elixirc_paths(:test), do: ["lib", "test/helpers"]
  defp elixirc_paths(_), do: ["lib"]

  # used by ex_doc
  defp docs do
    [
      main: "Ockam.ABAC",
      source_url_pattern:
        "#{@ockam_github_repo}/blob/v#{@version}/#{@ockam_github_repo_path}/%{path}#L%{line}"
    ]
  end

  defp aliases do
    [
      docs: "docs --output _build/docs --formatter html",
      "lint.format": "format --check-formatted",
      "lint.credo": "credo --strict",
      "lint.dialyzer": "dialyzer --format dialyxir",
      lint: ["lint.format", "lint.credo"],
      # test: "test --no-start",
      "test.cover": "test --no-start --cover",
      compile: ["compile", "compile_rules"]
    ]
  end
end
