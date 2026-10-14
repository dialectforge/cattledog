#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
Lu Brain v2 — Training Dashboard Launcher

Starts an HTTP server on port 8080, opens the training dashboard in the
browser, then launches train_with_progress.py on the Python PDF books.

Usage:
    python start_training.py

Requirements: No npm, no axios. Pure Python stdlib + browser fetch API.
"""

import json
import os
import subprocess
import sys
import threading
import time
import webbrowser
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

# ── Paths ──────────────────────────────────────────────────────────────────────

TOOLS_DIR      = Path(__file__).parent
DASHBOARD_HTML = TOOLS_DIR / "training_dashboard.html"
PROGRESS_FILE  = TOOLS_DIR / "training_progress.json"
TRAIN_SCRIPT   = TOOLS_DIR / "train_with_progress.py"
LOCK_FILE      = TOOLS_DIR / "training.lock"

PORT = 8080

# ── PDF books to train ─────────────────────────────────────────────────────────
# Think_Python_2e.pdf was trained in session 1 (195 para → 4,960 KUs).
# All remaining non-math PDFs listed below (119 books).
# Excluded (math-heavy): Mathematics_for_Machine_Learning, Open_Logic_Text_Complete,
#   forall_x_-_Intro_to_Formal_Logic, The_Modern_Mathematics_of_Deep_Learning,
#   Quantum_Computation_(Barenco_et_al.)

PDF_BOOKS = [
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/A_Comprehensive_Review_of_State-of-The-Art_Methods_for_Java_Code_Generation_from_Natural_Language_Text.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/A_Survey_of_AIOps_in_the_Era_of_Large_Language_Models.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/A_Survey_on_Multimodal_Large_Language_Models.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/A_Tutorial_about_Random_Neural_Networks_in_Supervised_Learning.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/A_Tutorial_on_Meta-Reinforcement_Learning.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/A_multitask_deep_learning_model_for_real-time_deployment_in_embedded_systems.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/A_rich_bounty_of_AGN_in_the_9_square_degree_Bootes_survey__high-z_obscured_AGN_and_large-scale_structure.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Adam_-_A_Method_for_Stochastic_Optimization.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Algorithms_by_Jeff_Erickson.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/An_Automated_Multiple-Choice_Question_Generation_Using_Natural_Language_Processing_Techniques.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/An_Image_is_Worth_16x16_Words_(ViT).pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/An_Open_Natural_Language_Processing_Development_Framework_for_EHR-based_Clinical_Research__A_case_demonstration_using_the_National_COVID_Cohort_Collaborative_(N3C).pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Architectural_Implications_of_Graph_Neural_Networks.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Attention_Is_All_You_Need.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Auto-Encoding_Variational_Bayes_(VAE).pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/BERT_-_Pre-training_of_Deep_Bidirectional_Transformers.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Bash_Notes_for_Professionals.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Batch_Normalization.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Beejs_Guide_to_C_Programming.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Beejs_Guide_to_Network_Programming.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Business_Ethics.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Business_Law_I_Essentials.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/CANDoSA__A_Hardware_Performance_Counter-Based_Intrusion_Detection_System_for_DoS_Attacks_on_Automotive_CAN_bus.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Causal-Paced_Deep_Reinforcement_Learning.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Co-Designed_Architectures_for_Modular_Superconducting_Quantum_Computers.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/College_Success.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Dalorex__A_Data-Local_Program_Execution_and_Architecture_for_Memory-bound_Applications.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/DeepCFL__Deep_Contextual_Features_Learning_from_a_Single_Image.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Deep_Learning_and_Computational_Physics_(Lecture_Notes).pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Deep_Residual_Learning_for_Image_Recognition_(ResNet).pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Deep_learning_observables_in_computational_fluid_dynamics.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Demystifying_Instruction_Mixing_for_Fine-tuning_Large_Language_Models.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Denoising_Diffusion_Probabilistic_Models.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Direct_Preference_Optimization_(DPO).pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Distilling_the_Knowledge_in_a_Neural_Network.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Efficient_Estimation_of_Word_Representations_(Word2Vec).pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Eloquent_JavaScript_4th_Edition.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Emotion_in_Reinforcement_Learning_Agents_and_Robots__A_Survey.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Entrepreneurship.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Exploring_Feasible_Design_Spaces_for_Heterogeneous_Constraints.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/FedCV__A_Federated_Learning_Framework_for_Diverse_Computer_Vision_Tasks.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Generative_Adversarial_Networks.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Git_Notes_for_Professionals.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/High-Resolution_Image_Synthesis_with_Latent_Diffusion.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/High-Resolution_Image_Synthesis_with_Latent_Diffusion_Models.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/How_ISO_C_became_unusable_for_operating_systems_development.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Insights_into_DeepSeek-V3__Scaling_Challenges_and_Reflections_on_Hardware_for_AI_Architectures.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Introduction_To_Computer_Science_-_WEB.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Introduction_to_Business.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Introduction_to_Philosophy.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Introduction_to_Programming_Using_Java_9th_ed.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/LLaMA_-_Open_and_Efficient_Foundation_Language_Models.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/LSQCA__Resource-Efficient_Load_Store_Architecture_for_Limited-Scale_Fault-Tolerant_Quantum_Computing.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/LaTeX_Notes_for_Professionals.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Language_Models_are_Few-Shot_Learners_(GPT-3).pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Layer_Normalization.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Learn_to_Accumulate_Evidence_from_All_Training_Samples__Theory_and_Practice.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Learning_From_Failure__Integrating_Negative_Examples_when_Fine-tuning_Large_Language_Models_as_Agents.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Linux_Notes_for_Professionals.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Llama_2_-_Open_Foundation_and_Fine-Tuned_Chat_Models.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Lu_P3_File_Format_Knowledge.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Masked_Conditional_Neural_Networks_for_Audio_Classification.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Modern_graph_neural_networks_do_worse_than_classical_greedy_algorithms_in_solving_combinatorial_optimization_problems_like_maximum_independent_set.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/MongoDB_Notes_for_Professionals.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/MySQL_Notes_for_Professionals.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Neural_Discrete_Representation_Learning_(VQ-VAE).pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Neural_Machine_Translation_by_Jointly_Learning_to_Align.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Neural_Machine_Translation_by_Jointly_Learning_to_Align_and_Translate.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Neural_Networks_and_Deep_Learning.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/OpenStax_American_Government_3e.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/OpenStax_Introduction_to_Sociology_3e.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Organizational_Behavior.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/PC_Assembly_Language.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Pointer_Networks.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/PostgreSQL_Notes_for_Professionals.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/PowerShell_Notes_for_Professionals.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Predicting_concentration_levels_of_air_pollutants_by_transfer_learning_and_recurrent_neural_network.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Principles-of-Data-Science-WEB.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Principles_of_Management.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Proximal_Policy_Optimization_(PPO).pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Pruning_Filters_for_Efficient_ConvNets.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Psychology_2e.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/PyramidTNT__Improved_Transformer-in-Transformer_Baselines_with_Pyramid_Architecture.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/RAFT_-_In_Search_of_an_Understandable_Consensus_Algorithm.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Realizing_Uncertainty-Aware_Timing_Stack_in_Embedded_Operating_System.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Reinforcement_Learning_-_An_Introduction_2nd_ed.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Reinforcement_Learning_Meets_Large_Language_Models__A_Survey_of_Advancements_and_Applications_Across_the_LLM_Lifecycle.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/SQL_Notes_for_Professionals.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Scaling_Laws_for_Neural_Language_Models.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Semi-Supervised_Classification_with_Graph_CNNs.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/SpeechPrompt__Prompting_Speech_Language_Models_for_Speech_Processing_Tasks.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Speech_and_Language_Processing_3rd_ed_draft.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Structure_and_Interpretation_of_Computer_Programs_(SICP).pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Swin_Transformer.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/The_Deep_Arbitrary_Polynomial_Chaos_Neural_Network_or_how_Deep_Artificial_Neural_Networks_could_benefit_from_Data-Driven_Homogeneous_Chaos_Theory.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/The_Design_of_Circuit-Measuring_Collaborative_Learning_System_with_Embedded_Broker.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/The_Methanol_Multibeam_Survey.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Think_Java_2e.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Towards_the_Study_of_Morphological_Processing_of_the_Tangkhul_Language.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Training_language_models_to_follow_instructions_(InstructGPT).pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Value_Bonuses_using_Ensemble_Errors_for_Exploration_in_Reinforcement_Learning.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Very_Deep_Convolutional_Networks_(VGGNet).pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/WiCV_2019__The_Sixth_Women_In_Computer_Vision_Workshop.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/Writing_Guide_with_Handbook.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/action_planning_guide.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/anthropic_sdk_and_claude_api_guide.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/browser_control_guide.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/cloud_infrastructure_guide.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/design_tools_guide.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/desktop_commander_guide.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/file_operations_guide.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/guide.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/image_and_vision_guide.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/mcp_tool_usage_guide.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/programming_for_tools_guide.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/shell_commands_guide.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/unity_mcp_guide.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/web_and_http_guide.pdf",
    r"<AI_FACTORY_ROOT>/tools/trainer/uploads/wsl_and_vms_guide.pdf",
]


# ── HTTP request handler ───────────────────────────────────────────────────────

class DashboardHandler(BaseHTTPRequestHandler):
    """Serves training_dashboard.html at / and progress JSON at /progress."""

    def log_message(self, fmt, *args):
        pass  # Suppress per-request console noise

    def do_GET(self):
        path = self.path.split("?")[0]  # strip query string

        if path in ("/", "/index.html"):
            self._serve_html()
        elif path == "/progress":
            self._serve_progress()
        else:
            self.send_response(404)
            self.end_headers()

    def _serve_html(self):
        try:
            content = DASHBOARD_HTML.read_bytes()
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(content)))
            self.end_headers()
            self.wfile.write(content)
        except FileNotFoundError:
            self.send_response(404)
            self.end_headers()
            self.wfile.write(b"training_dashboard.html not found")

    def _serve_progress(self):
        try:
            if PROGRESS_FILE.exists():
                content = PROGRESS_FILE.read_bytes()
            else:
                content = json.dumps({"status": "waiting", "books": []}).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Access-Control-Allow-Origin", "*")
            self.send_header("Cache-Control", "no-cache")
            self.send_header("Content-Length", str(len(content)))
            self.end_headers()
            self.wfile.write(content)
        except Exception as e:
            self.send_response(500)
            self.end_headers()
            self.wfile.write(str(e).encode())


# ── Server thread ──────────────────────────────────────────────────────────────

_server_ready = threading.Event()
_server_error: list[str] = []   # mutable container so the thread can write to it


def _run_server():
    try:
        server = HTTPServer(("localhost", PORT), DashboardHandler)
    except OSError as e:
        _server_error.append(str(e))
        _server_ready.set()   # unblock main thread so it can report the error
        return
    server.daemon_threads = True
    _server_ready.set()
    server.serve_forever()


# ── Main ───────────────────────────────────────────────────────────────────────

def main():
    print("=" * 65)
    print("  LU BRAIN v2 — TRAINING DASHBOARD LAUNCHER")
    print("=" * 65)

    # ── 0. Lock file guard — prevent multiple simultaneous training runs ───────
    if LOCK_FILE.exists():
        try:
            pid = int(LOCK_FILE.read_text().strip())
            # Check if that PID is still alive
            import ctypes
            handle = ctypes.windll.kernel32.OpenProcess(0x400, False, pid)
            alive = handle != 0
            if alive:
                ctypes.windll.kernel32.CloseHandle(handle)
        except Exception:
            alive = False

        if alive:
            print(f"\nERROR: Training is already running (PID {pid}).")
            print(f"       Lock file: {LOCK_FILE}")
            print("       Kill that process first, or delete the lock file manually.")
            sys.exit(1)
        else:
            print(f"  Stale lock file found (PID {pid} no longer running). Removing it.")
            LOCK_FILE.unlink(missing_ok=True)

    # Write our PID to the lock file
    LOCK_FILE.write_text(str(os.getpid()))

    # ── 1. Validate PDFs ─────────────────────────────────────────────────────
    print("\n[1/4] Checking PDF books...")
    existing_pdfs: list[str] = []
    for pdf in PDF_BOOKS:
        if os.path.isfile(pdf):
            size_mb = os.path.getsize(pdf) / (1024 * 1024)
            print(f"  [OK]      {os.path.basename(pdf)} ({size_mb:.1f} MB)")
            existing_pdfs.append(pdf)
        else:
            print(f"  [MISSING] {os.path.basename(pdf)}")

    if not existing_pdfs:
        print("\nERROR: No PDF books found. Aborting.")
        sys.exit(1)

    # ── 2. Write initial progress state ──────────────────────────────────────
    initial_state = {
        "status":     "starting",
        "started_at": time.time(),
        "current_book": 0,
        "books": [
            {
                "name":       os.path.basename(p),
                "path":       p,
                "status":     "queued",
                "paragraphs": 0,
                "ku_count":   0,
                "errors":     0,
                "rate":       0.0,
                "page":       0,
            }
            for p in existing_pdfs
        ],
    }
    with open(PROGRESS_FILE, "w", encoding="utf-8") as f:
        json.dump(initial_state, f, indent=2)
    print(f"\n  Progress file: {PROGRESS_FILE}")

    # ── 3. Start HTTP server ──────────────────────────────────────────────────
    print(f"\n[2/4] Starting HTTP server on http://localhost:{PORT} ...")
    server_thread = threading.Thread(target=_run_server, daemon=True)
    server_thread.start()
    _server_ready.wait(timeout=5.0)   # wait until bind succeeds or fails
    if _server_error:
        print(f"\nERROR: Could not bind port {PORT}: {_server_error[0]}")
        print("       Is another instance of start_training.py already running?")
        LOCK_FILE.unlink(missing_ok=True)
        sys.exit(1)
    print(f"  Server is up.")

    # ── 4. Open browser ───────────────────────────────────────────────────────
    url = f"http://localhost:{PORT}"
    print(f"\n[3/4] Opening browser: {url}")
    webbrowser.open(url)
    time.sleep(1.2)  # brief pause so browser has a head start

    # ── 5. Launch training subprocess ─────────────────────────────────────────
    print(f"\n[4/4] Launching training on {len(existing_pdfs)} book(s)...")
    print(f"      Script: {TRAIN_SCRIPT}")
    for pdf in existing_pdfs:
        print(f"      PDF:    {os.path.basename(pdf)}")
    print()

    cmd = (
        [sys.executable, str(TRAIN_SCRIPT),
         "--progress-file", str(PROGRESS_FILE)]
        + existing_pdfs
    )

    try:
        result = subprocess.run(cmd)
    except KeyboardInterrupt:
        print("\nInterrupted by user.")
        LOCK_FILE.unlink(missing_ok=True)
        sys.exit(0)
    finally:
        LOCK_FILE.unlink(missing_ok=True)

    # ── 6. Done ───────────────────────────────────────────────────────────────
    if result.returncode == 0:
        print("\nTraining finished successfully.")
    else:
        print(f"\nTraining exited with code {result.returncode}.")

    print(f"\nDashboard still running at {url}")
    print("Press Ctrl+C to stop the server.\n")

    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        print("Server stopped.")


if __name__ == "__main__":
    main()
